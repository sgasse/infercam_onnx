//! Neural network inference, pre- and postprocessing functions.
//!
//! This requires the `onnxruntime` library to be present in the library path of the system.
//! You can download a release for linux e.g.
//! [here](https://github.com/microsoft/onnxruntime/releases/tag/v1.9.0), unpack it and add the
//! `.so` files in a library path like `~/.local/lib`.

use image::{self, RgbImage};
use ndarray::s;
use smallvec::SmallVec;
use std::convert::TryInto;
use tract_onnx::prelude::*;

/// Positive additive constant to avoid divide-by-zero.
const EPS: f32 = 1.0e-7;

/// Initialize a model by the name and return a closure to its inference run function.
pub fn get_model_run_func(
    model_name: &str,
) -> Option<Box<dyn Fn(TVec<Tensor>) -> TractResult<TVec<Arc<Tensor>>>>> {
    let (file_name, input_fact) = match model_name {
        "ultraface-RFB-640" => (
            "version-RFB-640.onnx",
            InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, 480, 640)),
        ),
        "ultraface-RFB-320" => (
            "version-RFB-320.onnx",
            InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, 240, 320)),
        ),
        _ => return None,
    };

    let model = tract_onnx::onnx()
        .model_for_path(file_name)
        .expect("Model file not found")
        .with_input_fact(0, input_fact)
        .expect("Could not set input fact")
        .into_optimized()
        .expect("Could not optimize model")
        .into_runnable()
        .expect("Could not make model runnable");

    Some(Box::new(move |input_tensor| model.run(input_tensor)))
}

/// Get the preprocessing function for a model by the model name.
pub fn get_preproc_func(model_name: &str) -> Result<Box<dyn Fn(RgbImage) -> Tensor>, String> {
    let (width, height) = match model_name {
        "ultraface-RFB-640" => (640, 480),
        "ultraface-RFB-320" => (320, 240),
        _ => return Err(format!("Model {} not found", model_name)),
    };
    let preproc_func = move |image: RgbImage| {
        let resized: RgbImage = image::imageops::resize(
            &image,
            width,
            height,
            ::image::imageops::FilterType::Triangle,
        );

        let image: Tensor = tract_ndarray::Array4::from_shape_fn(
            (1, 3, height as usize, width as usize),
            |(_, c, y, x)| {
                // Note: Mean/std are from MobileNet, not from Ultraface, but work well
                let mean = [0.485, 0.456, 0.406][c];
                let std = [0.229, 0.224, 0.225][c];
                (resized[(x as _, y as _)][c] as f32 / 255.0 - mean) / std
            },
        )
        .into();
        image
    };

    Ok(Box::new(preproc_func))
}

/// Post-process the ultraface network output with sorting and non-maximum-suppression.
pub fn postproc_ultraface(result: SmallVec<[Arc<Tensor>; 4]>) -> Vec<([f32; 4], f32)> {
    let sorted_output = sort_ultraface_output_ascending(result);
    non_maximum_suppression(sorted_output, 0.5, 0.5)
}

/// Get the top most confident bounding box from the ultraface network output.
pub fn get_top_bbox_from_ultraface<'bbox_life>(
    result: SmallVec<[Arc<Tensor>; 4]>,
) -> ([f32; 4], f32) {
    let mut sorted_bboxes_with_confidences = sort_ultraface_output_ascending(result);
    sorted_bboxes_with_confidences.pop().unwrap()
}

/// Split and sort the ultraface network output in an ascending way.
///
/// The bounding box candidates with the highest confidence will be the last elements, allowing us
/// to take them by calling `pop()`.
/// This function clones the network output in the process of creating vectors. Avoiding this clone
/// and working with the raw network output could boost performance.
fn sort_ultraface_output_ascending(result: SmallVec<[Arc<Tensor>; 4]>) -> Vec<([f32; 4], f32)> {
    let mut confidences_face: Vec<f32> = result[0]
        .to_array_view::<f32>()
        .unwrap()
        .slice(s![0, .., 1])
        .iter()
        .cloned()
        .collect();

    let bboxes: Vec<f32> = result[1]
        .to_array_view::<f32>()
        .unwrap()
        .iter()
        .cloned()
        .collect::<Vec<f32>>();

    let mut bboxes: Vec<[f32; 4]> = bboxes.chunks(4).map(|x| x.try_into().unwrap()).collect();

    let mut bboxes_with_confidences: Vec<([f32; 4], f32)> =
        bboxes.drain(..).zip(confidences_face.drain(..)).collect();

    bboxes_with_confidences.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    bboxes_with_confidences
}

/// Run non-maximum-suppression on candidate bounding boxes.
///
/// Start with the most confident bounding box and iterate over all other bounding boxes in the
/// order of sinking confidence. Grow the vector of selected bounding boxes by adding only those
/// candidates which do not have a maximum IoU `max_iou` with already chosen bounding boxes. Stop
/// the computation at a minimum confidence score and discard all candidates less certain than
/// `min_confidence`.
fn non_maximum_suppression(
    mut sorted_bboxes_with_confidences: Vec<([f32; 4], f32)>,
    max_iou: f32,
    min_confidence: f32,
) -> Vec<([f32; 4], f32)> {
    let mut selected = vec![];
    'candidates: loop {
        // Get next most confident bbox from the back of ascending-sorted vector
        match sorted_bboxes_with_confidences.pop() {
            Some((bbox, confidence)) => {
                // Early exit when confidences are below what we expect
                if confidence < min_confidence {
                    break 'candidates;
                }

                // Check for overlap with any of the selected bboxes
                for (selected_bbox, _) in selected.iter() {
                    match iou(&bbox, selected_bbox) {
                        x if x > max_iou => continue 'candidates,
                        _ => (),
                    }
                }

                // bbox has no large overlap with any of the selected ones, add it
                selected.push((bbox, confidence))
            }
            None => break 'candidates,
        }
    }

    selected
}

/// Calculate the intersection-over-union metric for two bounding boxes.
fn iou(bbox_a: &[f32; 4], bbox_b: &[f32; 4]) -> f32 {
    // Calculate corner points of overlap box
    // If the boxes do not overlap, the corner-points will be ill defined, i.e. the top left
    // corner point will be below and to the right of the bottom right corner point. In this case,
    // the area will be zero.
    let overlap_box: [f32; 4] = [
        f32::max(bbox_a[0], bbox_b[0]),
        f32::max(bbox_a[1], bbox_b[1]),
        f32::min(bbox_a[2], bbox_b[2]),
        f32::min(bbox_a[3], bbox_b[3]),
    ];

    let overlap_area = bbox_area(&overlap_box);

    // Avoid division-by-zero with `EPS`
    overlap_area / (bbox_area(bbox_a) + bbox_area(bbox_b) - overlap_area + EPS)
}

/// Calculate the area enclosed by a bounding box.
///
/// The bounding box is passed as four-element array defining two points:
/// `[x_top_left, y_top_left, x_bottom_right, y_bottom_right]`
/// If the bounding box is ill-defined by having the bottom-right point above/to the left of the
/// top-left point, the area is zero.
fn bbox_area(bbox: &[f32; 4]) -> f32 {
    let width = bbox[3] - bbox[1];
    let height = bbox[2] - bbox[0];
    if width < 0.0 || height < 0.0 {
        // bbox is empty/undefined since the bottom-right corner is above the top left corner
        return 0.0;
    }

    width * height
}

#[cfg(test)]
mod tests {
    use super::{
        get_model_run_func, get_preproc_func, non_maximum_suppression,
        sort_ultraface_output_ascending,
    };
    use tract_onnx::prelude::tvec;

    /// Test the ultraface network inference with a set of known images with faces.
    #[test]
    fn run_ultraface_640_inference() {
        let model_name = "ultraface-RFB-640";
        let infer_func = get_model_run_func(model_name).unwrap();
        let preproc_func = get_preproc_func(model_name).unwrap();

        let images_with_num_faces = vec![
            ("test_pics/bruce-mars-ZXq7xoo98b0-unsplash.jpg", 3),
            ("test_pics/clarke-sanders-ybPJ47PMT_M-unsplash.jpg", 6),
            ("test_pics/helena-lopes-e3OUQGT9bWU-unsplash.jpg", 4),
            ("test_pics/kaleidico-d6rTXEtOclk-unsplash.jpg", 3),
            ("test_pics/michael-dam-mEZ3PoFGs_k-unsplash.jpg", 1),
            ("test_pics/mika-W0i1N6FdCWA-unsplash.jpg", 1),
            ("test_pics/omar-lopez-T6zu4jFhVwg-unsplash.jpg", 10),
        ];
        for (filename, expected_num_faces) in images_with_num_faces.iter() {
            let image = image::open(filename).unwrap().to_rgb8();

            let result = infer_func(tvec!(preproc_func(image))).unwrap();

            let sorted_output = sort_ultraface_output_ascending(result);
            let bboxes_with_confidences = non_maximum_suppression(sorted_output, 0.5, 0.5);

            let num_bboxes = bboxes_with_confidences.len() as i32;
            println!("Number of bboxes found: {:?}", num_bboxes);
            assert_eq!(num_bboxes, *expected_num_faces);
        }
    }

    #[test]
    fn test_nms_min_confidence() {
        let sorted_bboxes_with_confidences = vec![
            ([1.0, 1.0, 2.0, 2.0], 0.3), // this candidate should be filtered out
            ([3.0, 3.0, 4.0, 4.0], 0.8), // this candidate should be kept
        ];
        let filtered_bboxes_with_conf =
            non_maximum_suppression(sorted_bboxes_with_confidences, 0.5, 0.5);
        assert_eq!(filtered_bboxes_with_conf, vec![([3.0, 3.0, 4.0, 4.0], 0.8)]);
    }

    #[test]
    fn test_nms_max_iou() {
        let sorted_bboxes_with_confidences = vec![
            ([4.0, 4.0, 8.0, 8.0], 0.6), // this should be filtered due to too high IoU
            ([1.0, 1.0, 2.0, 2.0], 0.7), // unrelated candidate, should be kept
            ([5.5, 4.0, 8.5, 8.0], 0.9), // this is the first selected
        ];
        let filtered_bboxes_with_conf =
            non_maximum_suppression(sorted_bboxes_with_confidences, 0.5, 0.5);
        assert_eq!(
            filtered_bboxes_with_conf,
            vec![
                // the order is reversed
                ([5.5, 4.0, 8.5, 8.0], 0.9),
                ([1.0, 1.0, 2.0, 2.0], 0.7),
            ]
        );
    }

    #[test]
    fn test_nms_all_low_confidence() {
        let sorted_bboxes_with_confidences = vec![
            ([4.0, 4.0, 8.0, 8.0], 0.3),
            ([1.0, 1.0, 2.0, 2.0], 0.4),
            ([5.5, 4.0, 8.5, 8.0], 0.55),
        ];
        let filtered_bboxes_with_conf =
            non_maximum_suppression(sorted_bboxes_with_confidences, 0.5, 0.6);
        assert_eq!(filtered_bboxes_with_conf, vec![]);
    }

    #[test]
    fn test_nms_empty_input() {
        let sorted_bboxes_with_confidences = vec![];
        let filtered_bboxes_with_conf =
            non_maximum_suppression(sorted_bboxes_with_confidences, 0.5, 0.5);
        assert_eq!(filtered_bboxes_with_conf, vec![]);
    }
}

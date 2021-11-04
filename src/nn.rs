use image::{self, RgbImage};
use ndarray::s;
use smallvec::SmallVec;
use std::convert::TryInto;
use tract_onnx::prelude::*;

const EPS: f32 = 1.0e-7;

fn bbox_area(bbox: &[f32; 4]) -> f32 {
    let width = bbox[3] - bbox[1];
    let height = bbox[2] - bbox[0];
    if width < 0.0 || height < 0.0 {
        // bbox is empty/undefined since the bottom-right corner is above the top left corner
        return 0.0;
    }

    width * height
}

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

    overlap_area / (bbox_area(bbox_a) + bbox_area(bbox_b) - overlap_area + EPS)
}

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

fn non_maximum_suppression(
    mut sorted_bboxes_with_confidences: Vec<([f32; 4], f32)>,
    max_iou: f32,
    min_confidence: f32,
) -> Vec<([f32; 4], f32)> {
    if sorted_bboxes_with_confidences.len() < 2 {
        return sorted_bboxes_with_confidences;
    }

    // Choose most confident bbox (from the back of the ascending-sorted vector)
    let mut selected = vec![sorted_bboxes_with_confidences.pop().unwrap()];
    'candidates: loop {
        // Get next most confident bbox
        if let Some((bbox, confidence)) = sorted_bboxes_with_confidences.pop() {
            // Early exit when we get to low confidences
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
    }

    selected
}

pub fn postproc_ultraface(result: SmallVec<[Arc<Tensor>; 4]>) -> Vec<([f32; 4], f32)> {
    let sorted_output = sort_ultraface_output_ascending(result);
    non_maximum_suppression(sorted_output, 0.5, 0.5)
}

pub fn get_model_run_func(
    model_name: &str,
) -> Option<Box<dyn Fn(TVec<Tensor>) -> TractResult<TVec<Arc<Tensor>>>>> {
    let (file_name, input_fact) = match model_name {
        "ultraface-RFB-640" => (
            "ultraface-RFB-640.onnx",
            InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, 480, 640)),
        ),
        "ultraface-RFB-320" => (
            "ultraface-RFB-320.onnx",
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

pub fn get_top_bbox_from_ultraface<'bbox_life>(
    result: SmallVec<[Arc<Tensor>; 4]>,
) -> ([f32; 4], f32) {
    let mut sorted_bboxes_with_confidences = sort_ultraface_output_ascending(result);
    sorted_bboxes_with_confidences.pop().unwrap()
}

pub fn example() -> TractResult<()> {
    let model = tract_onnx::onnx()
        .model_for_path("ultraface-RFB-640.onnx")?
        .with_input_fact(
            0,
            InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, 480, 640)),
        )?
        .into_optimized()?
        .into_runnable()?;

    let image = image::open("test_pics/michael-dam-mEZ3PoFGs_k-unsplash.jpg")
        .unwrap()
        .to_rgb8();
    let resized =
        image::imageops::resize(&image, 640, 480, ::image::imageops::FilterType::Triangle);

    let image: Tensor = tract_ndarray::Array4::from_shape_fn((1, 3, 480, 640), |(_, c, y, x)| {
        let mean = [0.485, 0.456, 0.406][c];
        let std = [0.229, 0.224, 0.225][c];
        (resized[(x as _, y as _)][c] as f32 / 255.0 - mean) / std
    })
    .into();

    let result = model.run(tvec!(image))?;

    println!("Output shape: {:?}", result[0].shape());

    let best = result[0]
        .to_array_view::<f32>()?
        .iter()
        .cloned()
        .zip(2..)
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    println!("result: {:?}", best);
    Ok(())
}

// Library without "include" folder in `~/.local/lib`

#[cfg(test)]
mod tests {
    use crate::nn::{non_maximum_suppression, sort_ultraface_output_ascending};

    use super::{example, get_model_run_func, get_preproc_func};
    use tract_onnx::prelude::tvec;

    #[test]
    fn run_example() {
        let result = example();
        assert_eq!(result.unwrap(), ());
    }

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
}

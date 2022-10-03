use image::RgbImage;
use ndarray::s;
use smallvec::SmallVec;
use tract_onnx::prelude::*;

type Error = Box<dyn std::error::Error>;

type NnModel = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;
type NnOut = SmallVec<[Arc<Tensor>; 4]>;

/// Positive additive constant to avoid divide-by-zero.
const EPS: f32 = 1.0e-7;

pub trait InferModel {
    fn run(&self, input: RgbImage) -> Result<(), Error>;
}

pub struct UltrafaceModel {
    model: NnModel,
    width: u32,
    height: u32,
    max_iou: f32,
    min_confidence: f32,
}

impl UltrafaceModel {
    pub fn new() -> Result<Self, Error> {
        let model = get_ultraface_model()?;
        Ok(Self {
            model,
            width: 640,
            height: 480,
            // TODO: As input variable
            max_iou: 0.5,
            min_confidence: 0.5,
        })
    }

    fn preproc(&self, input: RgbImage) -> Tensor {
        let resized: RgbImage = image::imageops::resize(
            &input,
            self.width,
            self.height,
            // TODO: Test different filters
            image::imageops::FilterType::Triangle,
        );

        let tensor: Tensor = tract_ndarray::Array4::from_shape_fn(
            (1, 3, self.height as usize, self.width as usize),
            |(_, c, y, x)| {
                // Note: Mean/std are from MobileNet, not from Ultraface, but work well
                let mean = [0.485, 0.456, 0.406][c];
                let std = [0.229, 0.224, 0.225][c];
                (resized[(x as _, y as _)][c] as f32 / 255.0 - mean) / std
            },
        )
        .into();

        tensor
    }

    fn postproc(&self, raw_nn_out: NnOut) -> Result<(), Error> {
        // TODO: Document output
        let confidences = raw_nn_out[0]
            .to_array_view::<f32>()?
            .slice(s![0, .., 1])
            .to_vec();

        let bboxes: Vec<f32> = raw_nn_out[1]
            .to_array_view::<f32>()?
            .iter()
            .cloned()
            .collect();
        let bboxes: Vec<[f32; 4]> = bboxes.chunks(4).map(|x| x.try_into().unwrap()).collect();

        let mut confidences_with_bboxes: Vec<_> = confidences
            .iter()
            .zip(bboxes.iter())
            .filter_map(|(confidence, bbox)| match confidence {
                x if *x > self.min_confidence => Some((confidence, bbox)),
                _ => None,
            })
            .collect();

        confidences_with_bboxes.sort_by(|a, b| a.0.partial_cmp(b.0).unwrap());

        let selected_bboxes = non_maximum_suppression(confidences_with_bboxes, self.max_iou);

        dbg!(&selected_bboxes);

        Ok(())
    }
}

impl InferModel for UltrafaceModel {
    fn run(&self, input: RgbImage) -> Result<(), Error> {
        let valid_input = tvec!(self.preproc(input));
        let raw_nn_out = self.model.run(valid_input)?;
        self.postproc(raw_nn_out)?;

        Ok(())
    }
}

fn get_ultraface_model() -> Result<NnModel, Error> {
    let filename = "ultraface-RFB-640.onnx";
    let input_fact = InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, 480, 640));
    let model = tract_onnx::onnx()
        .model_for_path(filename)?
        .with_input_fact(0, input_fact)?
        .into_optimized()?
        .into_runnable()?;

    Ok(model)
}

/// Run non-maximum-suppression on candidate bounding boxes.
///
/// TODO: Overhaul
/// Start with the most confident bounding box and iterate over all other bounding boxes in the
/// order of sinking confidence. Grow the vector of selected bounding boxes by adding only those
/// candidates which do not have a maximum IoU `max_iou` with already chosen bounding boxes. Stop
/// the computation at a minimum confidence score and discard all candidates less certain than
/// `min_confidence`.
fn non_maximum_suppression(
    mut sorted_bboxes_with_confidences: Vec<(&f32, &[f32; 4])>,
    max_iou: f32,
) -> Vec<(f32, [f32; 4])> {
    let mut selected = vec![];
    'candidates: loop {
        // Get next most confident bbox from the back of ascending-sorted vector.
        // All boxes fulfill the minimum confidence criterium.
        match sorted_bboxes_with_confidences.pop() {
            Some((confidence, bbox)) => {
                // Check for overlap with any of the selected bboxes
                for (_, selected_bbox) in selected.iter() {
                    match iou(&bbox, selected_bbox) {
                        x if x > max_iou => continue 'candidates,
                        _ => (),
                    }
                }

                // bbox has no large overlap with any of the selected ones, add it
                selected.push((confidence.clone(), bbox.clone()))
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

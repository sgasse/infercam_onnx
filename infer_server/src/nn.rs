use crate::{utils::download_file, Error};
use image::RgbImage;
use ndarray::s;
use smallvec::SmallVec;
use tract_onnx::prelude::*;

type NnModel = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;
type NnOut = SmallVec<[Arc<Tensor>; 4]>;
type Bbox = [f32; 4];

/// Positive additive constant to avoid divide-by-zero.
const EPS: f32 = 1.0e-7;

const ULTRAFACE_LINK_640: &'static str = "https://github.com/onnx/models/raw/main/vision/body_analysis/ultraface/models/version-RFB-640.onnx";
const ULTRAFACE_LINK_320: &'static str = "https://github.com/onnx/models/raw/main/vision/body_analysis/ultraface/models/version-RFB-320.onnx";

pub trait InferModel {
    fn run(&self, input: RgbImage) -> Result<Vec<(Bbox, f32)>, Error>;
}

pub enum UltrafaceVariant {
    W640H480,
    W320H240,
}

pub struct UltrafaceModel {
    model: NnModel,
    width: u32,
    height: u32,
    max_iou: f32,
    min_confidence: f32,
}

impl UltrafaceModel {
    pub async fn new(variant: UltrafaceVariant) -> Result<Self, Error> {
        let (width, height) = match variant {
            UltrafaceVariant::W640H480 => (640, 480),
            UltrafaceVariant::W320H240 => (320, 240),
        };
        let model = Self::get_model(&variant, width, height).await?;
        println!("Initialized Ultraface model");

        Ok(Self {
            model,
            width,
            height,
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

    fn postproc(&self, raw_nn_out: NnOut) -> Result<Vec<(Bbox, f32)>, Error> {
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
        let bboxes: Vec<Bbox> = bboxes.chunks(4).map(|x| x.try_into().unwrap()).collect();

        let mut bboxes_with_confidences: Vec<_> = bboxes
            .iter()
            .zip(confidences.iter())
            .filter_map(|(bbox, confidence)| match confidence {
                x if *x > self.min_confidence => Some((bbox, confidence)),
                _ => None,
            })
            .collect();

        bboxes_with_confidences.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap());

        let selected_bboxes = non_maximum_suppression(bboxes_with_confidences, self.max_iou);

        Ok(selected_bboxes)
    }

    async fn get_model(
        variant: &UltrafaceVariant,
        width: u32,
        height: u32,
    ) -> Result<NnModel, Error> {
        let (model_name, download_link) = match variant {
            UltrafaceVariant::W640H480 => ("ultraface-RFB-640.onnx", ULTRAFACE_LINK_640),
            UltrafaceVariant::W320H240 => ("ultraface-RFB-320.onnx", ULTRAFACE_LINK_320),
        };

        let model_file_dir = dirs::cache_dir().expect("cache dir").join("infercam_onnx");
        if !model_file_dir.is_dir() {
            std::fs::create_dir(&model_file_dir)?;
        }

        let model_file_path = model_file_dir.join(model_name);
        if !model_file_path.is_file() {
            let client = reqwest::Client::new();
            println!("Downloading Ultraface model...");
            download_file(&client, download_link, &model_file_path).await?;
            println!("Download complete");
        }

        let input_fact =
            InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, height as i32, width as i32));
        let model = tract_onnx::onnx()
            .model_for_path(model_file_path)?
            .with_input_fact(0, input_fact)?
            .into_optimized()?
            .into_runnable()?;

        Ok(model)
    }
}

impl InferModel for UltrafaceModel {
    fn run(&self, input: RgbImage) -> Result<Vec<(Bbox, f32)>, Error> {
        let valid_input = tvec!(self.preproc(input));
        let raw_nn_out = self.model.run(valid_input)?;
        let selected_bboxes = self.postproc(raw_nn_out)?;

        Ok(selected_bboxes)
    }
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
    mut sorted_bboxes_with_confidences: Vec<(&Bbox, &f32)>,
    max_iou: f32,
) -> Vec<(Bbox, f32)> {
    let mut selected = vec![];
    'candidates: loop {
        // Get next most confident bbox from the back of ascending-sorted vector.
        // All boxes fulfill the minimum confidence criterium.
        match sorted_bboxes_with_confidences.pop() {
            Some((bbox, confidence)) => {
                // Check for overlap with any of the selected bboxes
                for (selected_bbox, _) in selected.iter() {
                    match iou(&bbox, selected_bbox) {
                        x if x > max_iou => continue 'candidates,
                        _ => (),
                    }
                }

                // bbox has no large overlap with any of the selected ones, add it
                selected.push((bbox.clone(), confidence.clone()))
            }
            None => break 'candidates,
        }
    }

    selected
}

/// Calculate the intersection-over-union metric for two bounding boxes.
fn iou(bbox_a: &Bbox, bbox_b: &Bbox) -> f32 {
    // Calculate corner points of overlap box
    // If the boxes do not overlap, the corner-points will be ill defined, i.e. the top left
    // corner point will be below and to the right of the bottom right corner point. In this case,
    // the area will be zero.
    let overlap_box: Bbox = [
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
fn bbox_area(bbox: &Bbox) -> f32 {
    let width = bbox[3] - bbox[1];
    let height = bbox[2] - bbox[0];
    if width < 0.0 || height < 0.0 {
        // bbox is empty/undefined since the bottom-right corner is above the top left corner
        return 0.0;
    }

    width * height
}

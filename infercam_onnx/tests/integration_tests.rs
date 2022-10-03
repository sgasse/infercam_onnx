use libinfercam_onnx::nn::{get_model_run_func, get_preproc_func, postproc_ultraface};
use tract_onnx::prelude::tvec;

/// Test the ultraface network inference with a set of known images with faces.
#[test]
fn test_ultraface_640_integration() {
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
        ("test_pics/ken-cheung-KonWFWUaAuk-unsplash.jpg", 0),
    ];
    for (filename, expected_num_faces) in images_with_num_faces.iter() {
        let image = image::open(filename).unwrap().to_rgb8();

        let result = infer_func(tvec!(preproc_func(image))).unwrap();

        let bboxes_with_confidences = postproc_ultraface(result);

        let num_bboxes = bboxes_with_confidences.len() as i32;
        println!("Number of bboxes found: {:?}", num_bboxes);
        assert_eq!(num_bboxes, *expected_num_faces);
    }
}

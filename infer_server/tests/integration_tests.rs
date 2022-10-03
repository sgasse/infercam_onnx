use infer_server::nn::{InferModel, UltrafaceModel};

#[test]
fn test_ultraface_640() -> Result<(), Box<dyn std::error::Error>> {
    dbg!(std::env::current_dir());
    let model = UltrafaceModel::new()?;

    let images_with_num_faces = vec![
        (
            "infercam_onnx/test_pics/bruce-mars-ZXq7xoo98b0-unsplash.jpg",
            3,
        ),
        (
            "infercam_onnx/test_pics/clarke-sanders-ybPJ47PMT_M-unsplash.jpg",
            6,
        ),
        (
            "infercam_onnx/test_pics/helena-lopes-e3OUQGT9bWU-unsplash.jpg",
            4,
        ),
        (
            "infercam_onnx/test_pics/kaleidico-d6rTXEtOclk-unsplash.jpg",
            3,
        ),
        (
            "infercam_onnx/test_pics/michael-dam-mEZ3PoFGs_k-unsplash.jpg",
            1,
        ),
        ("infercam_onnx/test_pics/mika-W0i1N6FdCWA-unsplash.jpg", 1),
        (
            "infercam_onnx/test_pics/omar-lopez-T6zu4jFhVwg-unsplash.jpg",
            10,
        ),
        (
            "infercam_onnx/test_pics/ken-cheung-KonWFWUaAuk-unsplash.jpg",
            0,
        ),
    ];
    for (filename, _expected_num_faces) in images_with_num_faces {
        let image = image::open(filename)?.to_rgb8();
        let result = model.run(image);
        dbg!(result);
    }

    Ok(())
}

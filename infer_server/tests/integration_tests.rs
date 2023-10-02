use std::path::Path;

use infer_server::nn::{InferModel, UltrafaceModel};

// #[tokio::test]
// async fn test_ultraface_640() -> Result<(), Box<dyn std::error::Error>> {
//     let current_workdir = std::env::current_dir()?;
//     println!("Running with workdir {}", current_workdir.display());
//     let model = UltrafaceModel::new(infer_server::nn::UltrafaceVariant::W640H480, 0.5, 0.5).await?;

//     // `cargo test` and debugging the test via IDE have differing work dirs
//     let test_pic_dir = {
//         let base_dir = "resources/test_pics";
//         match Path::new(base_dir).is_dir() {
//             true => base_dir.to_owned(),
//             false => format!("../{}", base_dir),
//         }
//     };

//     let images_with_num_faces = vec![
//         ("bruce-mars-ZXq7xoo98b0-unsplash.jpg", 3),
//         ("clarke-sanders-ybPJ47PMT_M-unsplash.jpg", 6),
//         ("helena-lopes-e3OUQGT9bWU-unsplash.jpg", 4),
//         ("kaleidico-d6rTXEtOclk-unsplash.jpg", 3),
//         ("michael-dam-mEZ3PoFGs_k-unsplash.jpg", 1),
//         ("mika-W0i1N6FdCWA-unsplash.jpg", 1),
//         ("omar-lopez-T6zu4jFhVwg-unsplash.jpg", 10),
//         ("ken-cheung-KonWFWUaAuk-unsplash.jpg", 0),
//     ];
//     for (filename, expected_num_faces) in images_with_num_faces {
//         let image = image::open(Path::new(&test_pic_dir).join(Path::new(filename)))?.to_rgb8();
//         let bboxes_with_confidences = model.run(image)?;

//         assert_eq!(bboxes_with_confidences.len(), expected_num_faces);
//     }

//     Ok(())
// }

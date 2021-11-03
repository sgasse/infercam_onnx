use image::{self, ImageBuffer, Rgb};
use tract_onnx::prelude::*;

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

pub fn get_preproc_func(
    model_name: &str,
) -> Result<Box<dyn Fn(ImageBuffer<Rgb<u8>, Vec<u8>>) -> Tensor>, String> {
    let (width, height) = match model_name {
        "ultraface-RFB-640" => (640, 480),
        "ultraface-RFB-320" => (320, 240),
        _ => return Err(format!("Model {} not found", model_name)),
    };
    let preproc_func = move |image: ImageBuffer<Rgb<u8>, Vec<u8>>| {
        let resized: ImageBuffer<Rgb<u8>, Vec<u8>> = image::imageops::resize(
            &image,
            width,
            height,
            ::image::imageops::FilterType::Triangle,
        );

        let image: Tensor = tract_ndarray::Array4::from_shape_fn(
            (1, 3, height as usize, width as usize),
            |(_, c, y, x)| {
                // TODO: Real mean and std?
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

pub fn example() -> TractResult<()> {
    let model = tract_onnx::onnx()
        .model_for_path("ultraface-RFB-640.onnx")?
        .with_input_fact(
            0,
            InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, 480, 640)),
        )?
        .into_optimized()?
        .into_runnable()?;

    let image = image::open("grace_hopper.jpg").unwrap().to_rgb8();
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
    use super::{example, get_model_run_func, get_preproc_func};
    use tract_onnx::prelude::tvec;

    #[test]
    fn run_example() {
        let res = example();
        assert_eq!(res.unwrap(), ());
        println!("Test done");
    }

    #[test]
    fn run_ultraface_640_inference() {
        let model_name = "ultraface-RFB-640";
        let infer_func = get_model_run_func(model_name).unwrap();
        let preproc_func = get_preproc_func(model_name).unwrap();

        let image = image::open("grace_hopper.jpg").unwrap().to_rgb8();

        let result = infer_func(tvec!(preproc_func(image))).unwrap();
        for (index, output) in result.iter().enumerate() {
            println!("Output {} has shape {:?}", index, output.shape());
        }
    }

    #[test]
    fn run_ultraface_320_inference() {
        let model_name = "ultraface-RFB-320";
        let infer_func = get_model_run_func(model_name).unwrap();
        let preproc_func = get_preproc_func(model_name).unwrap();

        let image = image::open("grace_hopper.jpg").unwrap().to_rgb8();

        let result = infer_func(tvec!(preproc_func(image))).unwrap();
        for (index, output) in result.iter().enumerate() {
            println!("Output {} has shape {:?}", index, output.shape());
        }
    }
}

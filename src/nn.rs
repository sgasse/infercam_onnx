use image;
use tract_onnx::prelude::*;

pub fn run() -> TractResult<()> {
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
    use super::run;

    #[test]
    fn run_inference() {
        let res = run();
        assert_eq!(res.unwrap(), ());
        println!("Test done");
    }
}

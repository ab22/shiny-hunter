fn main() -> Result<(), opencv::Error> {
    println!("OpenCV version: {}", opencv::core::get_version_string()?);

    Ok(())
}

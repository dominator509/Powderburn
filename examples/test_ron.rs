use serde::Deserialize;
#[derive(Debug, Deserialize)]
struct Weapon { id: String, accuracy: i32, range_bands: [i32; 4] }
fn main() {
    let s = r"Weapon(id: \"test\", accuracy: 5, range_bands: [1, 2, 3, 4])";
    let w: Weapon = ron::from_str(s).unwrap();
    println!("{:?}", w);
}

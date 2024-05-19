use anyhow::Result;

use serde::{Deserialize, Serialize};
use strum::{
    Display, EnumCount, EnumDiscriminants, EnumIs, EnumIter, EnumString, IntoEnumIterator,
    IntoStaticStr, VariantNames,
};

#[allow(unused)]
#[derive(Display, Debug, Serialize, Deserialize)]
enum Color {
    #[strum(serialize = "redred", to_string = "red")]
    Red,
    Green {
        range: usize,
    },
    Blue(usize),
    Yellow,
    #[strum(to_string = "purple with {sat} saturation")]
    Purple {
        sat: usize,
    },
}

#[derive(
    Debug, EnumString, EnumCount, EnumDiscriminants, EnumIter, EnumIs, IntoStaticStr, VariantNames,
)]
#[allow(unused)]
enum MyEnum {
    A,
    B(String),
    C,
    D,
}
fn main() -> Result<()> {
    println!("{:?}", MyEnum::VARIANTS);
    MyEnum::iter().for_each(|v| println!("{:? }", v));
    let my_num = MyEnum::B("hello".to_string());
    println!("total variants: {:?}", MyEnum::COUNT);
    println!("{:?}", my_num.is_b());
    let s: &'static str = my_num.into();
    println!("{:?}", s);

    let red = Color::Red;
    let green = Color::Green { range: 5 };
    let blue = Color::Blue(10);
    let yellow = Color::Yellow;
    let purple = Color::Purple { sat: 100 };

    println!(
        "red:{} green:{} blue:{} yellow:{} purple:{}",
        red, green, blue, yellow, purple
    );

    let red_str = serde_json::to_string(&red)?;
    println!("{:?}", red_str);
    Ok(())
}

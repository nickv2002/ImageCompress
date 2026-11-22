fn main(){let a:Vec<String>=std::env::args().collect();let im=image::open(&a[1]).unwrap();im.to_rgb8().save(&a[2]).unwrap();}

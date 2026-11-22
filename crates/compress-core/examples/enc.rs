use compress_core::jpeg_codec::*;
fn main(){let a:Vec<String>=std::env::args().collect();let im=image::open(&a[1]).unwrap().to_rgb8();let (w,h)=im.dimensions();
for (name,q,sub,prog) in [("q85 444 p2",Some(85u8),Subsampling::S444,2),("q85 444 p0",Some(85),Subsampling::S444,0),("dist1.45",None,Subsampling::S444,2)]{
let b=encode_rgb8_jpegli(&im,w,h,&JpegliOpts{distance:1.45,quality:q,progressive_level:prog,subsampling:sub,icc:None}).unwrap();println!("{name}: {}",b.len());}}

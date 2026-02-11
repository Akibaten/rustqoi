use std::fs::write;
use std::time::Instant;
use std::path::Path;
use image::ImageReader;
use std::env;

fn pos_hash(red:u8, green:u8, blue:u8, alpha:u8) -> usize { 
    let pos:u8 = (((red as u32)*3+(green as u32)*5+(blue as u32)*7+(alpha as u32)*11) % 64) as u8;
    return pos as usize
}

fn qoi_op_rgb(data_stream: &mut Vec<u8>, red:u8, green:u8, blue:u8) -> () {
    data_stream.push(0xfe);
    data_stream.push(red);
    data_stream.push(green);
    data_stream.push(blue);
}

fn qoi_op_rgba(data_stream: &mut Vec<u8>, red:u8, green:u8, blue:u8, alpha:u8) -> () {
    data_stream.push(0xff);
    data_stream.push(red);
    data_stream.push(green);
    data_stream.push(blue);
    data_stream.push(alpha)
}

fn qoi_op_index(data_stream: &mut Vec<u8>, index:u8) -> (){
    data_stream.push(index & 0x3f);
}

fn qoi_op_run(data_stream: &mut Vec<u8>, run:u8) -> () {
    data_stream.push(0b11000000 | ((run-1) & 0x3f));
}

fn qoi_op_diff(data_stream: &mut Vec<u8>, dr:u8, dg:u8, db:u8) -> () {
    let byte = 0b01000000 | ((dr & 0x03) << 4) | ((dg & 0x03) << 2) | (db & 0x03);
    data_stream.push(byte);
}

fn qoi_op_luma(data_stream: &mut Vec<u8>, dg:u8, dr_dg:u8, db_dg:u8) -> () {
    let byte = (dr_dg & 0x0f) << 4 | (db_dg & 0x0f); 
    data_stream.push(0b10000000 | dg);
    data_stream.push(byte);
}

fn write_qoi_header(data_stream: &mut Vec<u8>, width:u32, height:u32, channels:u8, color:u8) -> (){
    data_stream.extend_from_slice(b"qoif");
    data_stream.extend_from_slice(&width.to_be_bytes());
    data_stream.extend_from_slice(&height.to_be_bytes());
    data_stream.push(channels);
    data_stream.push(color);
}

fn main() {
    let start = Instant::now();
    //lets us use paths from the command line arguments
    let args: Vec<String> = env::args().collect();
    let path = Path::new(&args[1]);

    let mut data_stream = Vec::<u8>::new();
    let mut color_index: [Option<[u8;3]>;64] = [None; 64];

    //test image
    let img = ImageReader::open(path)
        .unwrap()
        .decode()
        .unwrap()
        .to_rgb8();
    
    let pixels: Vec<u8> = img.as_raw().to_vec();
    let width = img.width();
    let height = img.height();

    //header
    write_qoi_header(&mut data_stream, width, height, 3, 0);
    let mut diff = 0;
    let mut runs = 0;
    let mut rgbs = 0;
    let mut lumas = 0;
    let mut indexes = 0;
    let mut hash: usize;
    let mut rgb:[u8;3];
    let mut i = 0;
    let mut prev_pixel = [0u8,0u8,0u8];
    let mut dc:[u8;3];
    while i < (pixels.len()/3){
        rgb = [pixels[i * 3], pixels[i * 3 + 1], pixels[i * 3 + 2]];
        hash = pos_hash(rgb[0], rgb[1], rgb[2], 255);
       
        //dc is array for [dr,dg,db]
        dc = [
           rgb[0].wrapping_sub(prev_pixel[0]) as u8,
           rgb[1].wrapping_sub(prev_pixel[1]) as u8,
           rgb[2].wrapping_sub(prev_pixel[2]) as u8
        ];
        // qoi run chunk check
        if rgb == prev_pixel{
            let mut run = 0;
            while i < pixels.len()/3
                &&[pixels[i*3],pixels[i*3+1],pixels[i*3+2]] == rgb 
                && run < 62{
                run += 1;
                runs += 1;
                i += 1;
            }
            qoi_op_run(&mut data_stream, run);
            prev_pixel = rgb;
            continue;
        }

        //qoi index check
        if color_index[hash].is_none(){
            color_index[hash] = Some(rgb);
            qoi_op_rgb(&mut data_stream, rgb[0], rgb[1], rgb[2]);
            rgbs += 1;
        }else{
            if color_index[hash] == Some(rgb){
                qoi_op_index(&mut data_stream, hash as u8);
                indexes += 1;
            }else{
                // //qoi diff check
                if dc[0].wrapping_add(2) <= 3
                && dc[1].wrapping_add(2) <= 3
                && dc[2].wrapping_add(2) <= 3
                {
                    // dbg!(prev_pixel,rgb);
                    // dbg!(dc);
                    diff += 1;
                    qoi_op_diff(&mut data_stream,dc[0].wrapping_add(2),dc[1].wrapping_add(2),dc[2].wrapping_add(2));
                    color_index[hash] = Some(rgb);
                    i += 1;
                    prev_pixel = rgb;
                    continue
                }


                //qoi luma check
                if dc[1] as i8 <= 31 && dc[1] as i8 >= -32{
                    let dr_dg = dc[0].wrapping_sub(dc[1]);
                    let db_dg = dc[2].wrapping_sub(dc[1]);
                    if dr_dg as i8 >= -8 && dr_dg as i8 <= 7 && db_dg as i8 >= -8 && db_dg as i8 <= 7{
                        qoi_op_luma(&mut data_stream,
                            dc[1].wrapping_add(32),
                            dr_dg.wrapping_add(8),
                            db_dg.wrapping_add(8));
                        i += 1;
                        lumas += 1;
                        color_index[hash] = Some(rgb);
                        prev_pixel = rgb;
                        continue
                    }
                }

                //qoi rgb if no other valid chunk
                qoi_op_rgb(&mut data_stream, rgb[0], rgb[1], rgb[2]);
                color_index[hash] = Some(rgb);
                rgbs += 1
            }
        }
        prev_pixel = rgb;
        i += 1;
    }

    //end data_stream
    for _ in 0..7{
        data_stream.push(0x00);
    }
    data_stream.push(0x01);

    //write to file
    write(path.with_extension("qoi"),data_stream).unwrap();

    println!("run:{} diff:{} index:{} rgb:{} luma:{}", runs, diff, indexes, rgbs, lumas);
    println!("time elapsed: {:?}", start.elapsed());
}

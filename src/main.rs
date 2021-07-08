#![allow(dead_code)]

#[macro_use]
extern crate quick_from;

use std::io;
use std::rc::Rc;


use geo::algorithm::winding_order::Winding;

use geo::{
    MultiPolygon,
    polygon,
};

use geo_booleanop::boolean::BooleanOp;

use clap::{Arg, App, SubCommand, AppSettings};
use ttf_parser::{
    self as ttf,
    Face
};

use tiny_skia as skia;
use usvg::NodeExt;


fn arg_font() -> Arg<'static, 'static> {
    Arg::with_name("font")
        .short("f")
        .long("font")
        .value_name("font")
        .required(true)
        .help("The font file to try, only ttf is currently supported")
}


fn main() {
    let matches = App::new("font-pixelator")
        .version("0.1.0")
        .author("ear7h")
        .about("Pixelates fonts")
        .setting(AppSettings::SubcommandRequired)
        .subcommand(
            SubCommand::with_name("test")
                .about("Writes a test image of the pixelatedfont")
                .arg(arg_font())
                .arg(
                    Arg::with_name("output")
                        .short("o")
                        .long("output")
                        .value_name("out.png")
                        .help("Output file, png and svg are supported")
                        .required(true))
                .arg(
                    Arg::with_name("index")
                        .long("index")
                        .default_value("0")
                        .help("The index of the font in a collection")
                        .required(true))
                .arg(
                    Arg::with_name("anti_alias")
                        .short("a")
                        .long("anti-alias")
                        .help("turn on anti-aliasing"))
                .arg(
                    Arg::with_name("bolden")
                        .long("bolden")
                        .takes_value(true)
                        .help("Boldens the text"))
                .arg(
                    Arg::with_name("obliquen")
                        .long("obliquen")
                        .takes_value(true)
                        .help("Obliquens the text"))
                .arg(
                    Arg::with_name("bbox_width")
                        .long("bbox-width")
                        .takes_value(true)
                        .help("Override the width of a character"))
                .arg(
                    Arg::with_name("bbox_height")
                        .long("bbox-height")
                        .takes_value(true)
                        .help("Override the height of a character"))
                .arg(
                    Arg::with_name("pixels_per_em")
                        .long("pixels-per-em")
                        .default_value("50")
                        .help("Set the number of pixels for an em"))
                .arg(
                    Arg::with_name("text")
                        .help("Sample text for test output")
                        .required(true)
                        .index(1)))
        .get_matches();

    match matches.subcommand() {
        ("test", Some(submatches)) => {
            let bbox_width = submatches
                .value_of("bbox_width")
                .map(|x| x.parse().unwrap());

            let bbox_height = submatches
                .value_of("bbox_height")
                .map(|x| x.parse().unwrap());

            let obliquen = submatches
                .value_of("obliquen")
                .map(|x| x.parse().unwrap());

            let bolden = submatches
                .value_of("bolden")
                .map(|x| x.parse().unwrap());

            let pixels_per_em = submatches
                .value_of("pixels_per_em")
                .unwrap()
                .parse()
                .unwrap();

            CmdTest{
                font_file : submatches.value_of("font").unwrap().to_string(),
                output : submatches.value_of("output").unwrap().to_string(),
                text : submatches.value_of("text").unwrap().to_string(),
                index : submatches.value_of("index").unwrap().parse().unwrap(),
                anti_alias : submatches.value_of("anti_alias").is_some(),
                obliquen,
                bolden,
                pixels_per_em,
                bbox_width,
                bbox_height,
            }.run().unwrap();

        },
        _ => {
            println!("what did you do??")
        }
    }
}



#[derive(Debug, QuickFrom)]
enum Error {
    NoGlyph(char),
    InvalidPath,
    DrawingGlyph,
    FillPath,

    #[quick_from]
    Io(io::Error),
    #[quick_from]
    Ttf(ttf::FaceParsingError),
    #[quick_from]
    PngEncode(png::EncodingError)
}

type Result<T> = std::result::Result<T, Error>;


struct CmdTest {
    font_file : String,
    output : String,
    text : String,
    index : u32,
    pixels_per_em : f32,
    bbox_width : Option<f32>,
    bbox_height : Option<f32>,
    bolden : Option<f32>,
    obliquen : Option<f32>,
    anti_alias : bool
}


impl CmdTest {
    fn run(&self) -> Result<()> {
        let CmdTest{
            font_file,
            output,
            text,
            pixels_per_em,
            ..
        } = self;

        let font_bytes = std::fs::read(&font_file)?;
        let face = Face::from_slice(&font_bytes, self.index)?;

        let mut cursor = OutlineBuilderCursor {
            inner : OutlineBuilder{
                inner : skia::PathBuilder::new(),
            },
            x : 0.0,
            y : 0.0,
        };

        let scale = pixels_per_em / face.units_per_em().unwrap() as f32;
        let bbox = face.global_bounding_box();
        dbg!(bbox);
        let width = self.bbox_width.unwrap_or(bbox.width() as f32);
        let height = self.bbox_height.unwrap_or(bbox.height() as f32);
        let trans = skia::Transform::from_scale(scale, -scale)
            .pre_translate(0.0, -height);

        for ch in text.chars() {
            match ch {
                '\n' | '\r' => {
                    cursor.new_line(height);
                },
                ' ' => {
                    cursor.advance(width, 0.0);
                },
                _ => {
                    let glyph_id = face
                        .glyph_index(ch)
                        .ok_or(Error::NoGlyph(ch))?;

                    if self.bolden.or(self.obliquen).is_some() {
                        let mut utils = ttf_utils::Outline::new(&face, glyph_id)
                            .ok_or(Error::DrawingGlyph)?;

                        self.bolden.map(|x| {
                            utils.embolden(-x);
                        });

                        self.obliquen.map(|x| {
                            utils.oblique(x);
                        });

                        utils.emit(&mut cursor);

                    } else {
                        face
                            .outline_glyph(glyph_id, &mut cursor)
                            .ok_or(Error::DrawingGlyph)?;
                    }

                    cursor.advance(width, 0.0);

                }
            }
        }

        let path = cursor.inner.inner
            .finish()
            .ok_or(Error::InvalidPath)?
            .transform(trans)
            .unwrap();


        let mut paint = skia::Paint::default();
        paint.anti_alias = self.anti_alias;

        let bbox = path.bounds();
        println!("{:?}", path);
        println!("{:?}", bbox);

        let mut pixmap = skia::Pixmap::new(
            bbox.right() as u32 + 10,
            bbox.bottom() as u32 + 10,
        ).unwrap();
        pixmap.fill(skia::Color::WHITE);

        pixmap.fill_path(
            &path,
            &paint,
            skia::FillRule::Winding,
            //skia::Transform::from_scale(10.0, 10.0),
            skia::Transform::identity(),
            None
            ).ok_or(Error::FillPath)?;

        let ext = std::path::Path::new(&self.output)
            .extension()
            .unwrap()
            .to_str()
            .unwrap();
        match ext {
            "png" => {
                pixmap
                    .save_png(output)?;

                Ok(())
            },
            "svg" => {


                let svg_tree = usvg::Tree::create(
                    usvg::Svg {
                        size : usvg::Size::new(200.0, 100.0).unwrap(),
                        view_box : usvg::ViewBox {
                            rect : usvg::Rect::new(
                                0.0, 0.0,
                                pixmap.width().into(), pixmap.height().into())
                                .unwrap(),
                            aspect : usvg::AspectRatio::default(),
                        }
                    });

                let path = multipoly2svg(&mut multi_poly_from_pixels(&pixmap));

                svg_tree.root().append_kind(usvg::NodeKind::Path(usvg::Path{
                    fill : Some(Default::default()),
                    /*
                    stroke : Some(usvg::Stroke {
                        paint : usvg::Paint::Color(usvg::Color::red()),
                        width : usvg::StrokeWidth::new(0.1),
                        .. Default::default()
                    }),
                    */
                    data : Rc::new(path),
                    .. Default::default()
                }));

                let s = svg_tree.to_string(usvg::XmlOptions::default());
                std::fs::write(&output, &s)?;

                return Ok(())
            },
            _ => {
                // extension should be checked for validity befor
                // any work is done
                todo!();
            }
        }
    }

}

/// needs mut to correct winding orders
fn multipoly2svg(multi_poly : &mut MultiPolygon<f32>) -> usvg::PathData {
    let mut path = usvg::PathData::new();

    for poly in multi_poly.0.iter_mut() {
        poly.exterior_mut(|ls| {
            ls.make_cw_winding();
        });

        let mut points = poly.exterior().points_iter();
        if let Some(point) = points.next() {
            let (x, y) = point.x_y();
            path.push_move_to(x as f64, y as f64);
        }

        for point in points {
            let (x, y) = point.x_y();
            path.push_line_to(x as f64, y as f64);
        }

        poly.interiors_mut(|lss| {
            for ls in lss {
                ls.make_ccw_winding();
            }
        });

        for interior in poly.interiors() {
            let mut points = interior.points_iter();
            if let Some(point) = points.next() {
                let (x, y) = point.x_y();
                path.push_move_to(x as f64, y as f64);
            }

            for point in points {
                let (x, y) = point.x_y();
                path.push_line_to(x as f64, y as f64);
            }

        }
    }

    path
}

fn pixel_sum(p : &skia::PremultipliedColorU8) -> u32 {
    p.red() as u32 +
        p.green() as u32 +
        p.blue() as u32 +
        p.alpha() as u32
}

fn multi_poly_from_pixels(pixmap : &skia::Pixmap) -> MultiPolygon<f32> {

    let width = pixmap.width();
    let mut poly = MultiPolygon(Vec::new());

    for (idx, pixel) in pixmap.pixels().iter().enumerate() {
        let x = (idx as u32 % width) as f32;
        let y = (idx as u32 / width) as f32;

        if pixel_sum(pixel) < 270 {
            let square = polygon![
                (x: x,   y: y),
                (x: x,   y: y-1.),
                (x: x+1., y: y-1.),
                (x: x+1., y: y),
            ];

            poly = poly.union(&square);
        }
    }

    poly
}




struct OutlineBuilder {
    inner : skia::PathBuilder,
}

impl ttf::OutlineBuilder for OutlineBuilder {
    #[inline(always)]
    fn move_to(&mut self, x: f32, y: f32) {
        self.inner.move_to(x, y);
    }

    #[inline(always)]
    fn line_to(&mut self, x: f32, y: f32) {
        self.inner.line_to(x, y);
    }

    #[inline(always)]
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.inner.quad_to(x1, y1, x, y);
    }

    #[inline(always)]
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.inner.cubic_to(x1, y1, x2, y2, x, y);
    }

    #[inline(always)]
    fn close(&mut self) {
        self.inner.close();
    }
}

struct OutlineBuilderCursor<T> {
    inner : T,
    x : f32,
    y : f32,
}

impl <P> OutlineBuilderCursor<P> {
    fn advance(&mut self, dx : f32, dy : f32) {
        self.x += dx;
        self.y += dy;
    }

    fn new_line(&mut self, dy : f32) {
        self.x = 0.0;
        self.y -= dy;
    }
}

impl <P> ttf::OutlineBuilder for OutlineBuilderCursor<P>
where
    P : ttf::OutlineBuilder
{
    #[inline(always)]
    fn move_to(&mut self, x: f32, y: f32) {
        self.inner.move_to(x + self.x, y + self.y);
    }

    #[inline(always)]
    fn line_to(&mut self, x: f32, y: f32) {
        self.inner.line_to(x + self.x, y + self.y);
    }

    #[inline(always)]
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.inner.quad_to(
            x1 + self.x, y1 + self.y,
            x + self.x, y + self.y);
    }

    #[inline(always)]
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.inner.curve_to(
            x1 + self.x, y1 + self.y,
            x2 + self.x, y2 + self.y,
            x + self.x, y + self.y);
    }

    #[inline(always)]
    fn close(&mut self) {
        self.inner.close();
    }
}

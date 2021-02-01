// Shamelessly stolen from https://github.com/Geal/nom
#![allow(dead_code)]
extern crate nom;
extern crate rand;

use image::{DynamicImage, Rgba};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut};
use nom::{
    bytes::complete::{tag, take_while_m_n},
    combinator::map_res,
    sequence::tuple,
    IResult,
};
use rand::seq::SliceRandom;
use rand::Rng;
use rusttype::{point, Font, Scale};
use std::{env, fmt};

#[derive(Debug, PartialEq, Default)]
pub struct Point2D {
    x: i32,
    y: i32,
}

impl Point2D {
    fn distance(&self, other: &Point2D) -> f64 {
        (((other.x - self.x).pow(2) + (other.y - self.y).pow(2)) as f64).sqrt()
    }
}

impl fmt::Display for Point2D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[derive(Debug, PartialEq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str_radix(input, 16)
}

fn is_hex_digit(c: char) -> bool {
    c.is_digit(16)
}

fn hex_primary(input: &str) -> IResult<&str, u8> {
    map_res(take_while_m_n(2, 2, is_hex_digit), from_hex)(input)
}

fn hex_color(input: &str) -> IResult<&str, Color> {
    let (input, _) = tag("#")(input)?;
    let (input, (red, green, blue)) = tuple((hex_primary, hex_primary, hex_primary))(input)?;

    Ok((input, Color { red, green, blue }))
}

enum IshiharaColor {
    Inside,
    Outside,
}

struct Circle {
    center: Point2D,
    radius: f64,
    ishihara_color: Option<IshiharaColor>,
}

impl fmt::Display for Circle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}", self.center, self.radius)
    }
}

//Red, Red, Orange, Yellow, Light Red, Light Red, Tan
const RED_GREEN_INSIDE: &[&str] = &[
    "#cf5f47", "#cf5f47", "#fd9500", "#ffd500", "#ee8568", "#ee8568", "#eebd7a",
];

//Dark Green, Green, Light Green
const RED_GREEN_OUTSIDE: &[&str] = &["#5a8a50", "#a2ab5a", "#c9cc7d"];

impl Circle {
    pub const MAX_RADIUS: f64 = 6.9;
    pub const MIN_RADIUS: f64 = 3.0;
    fn create_circles(x: u32, y: u32, rng: &mut rand::rngs::ThreadRng) -> Vec<Circle> {
        let goal_area_ratio: f64 = 0.57;
        let goal_area = goal_area_ratio * x as f64 * y as f64;
        let mut circles: Vec<Circle> = Vec::new();
        let mut area: f64 = 0.0;

        //Create circles with random coordinates and radii with size based on its distance from the closest circle
        while area < goal_area {
            let candidate_point = Point2D {
                x: rng.gen_range(0..x) as i32,
                y: rng.gen_range(0..y) as i32,
            };

            if let Some(radius) = max_allowed_radius(&candidate_point, &circles, rng) {
                area += std::f64::consts::PI * radius.powi(2) as f64;
                let new_circle = Circle {
                    center: candidate_point,
                    radius,
                    ishihara_color: None,
                };
                circles.push(new_circle);
            }
        }
        circles
    }

    fn assign_color(&mut self, image: &image::RgbaImage) {
        let pixel = image.get_pixel(self.center.x as u32, self.center.y as u32);
        if pixel.0 == [0, 0, 0, 0] {
            self.ishihara_color = Some(IshiharaColor::Inside);
        } else {
            self.ishihara_color = Some(IshiharaColor::Outside);
        }
    }

    fn draw(&self, image: &mut image::RgbaImage, rng: &mut rand::rngs::ThreadRng) {
        let (_remainder, color) = match self.ishihara_color {
            Some(IshiharaColor::Inside) => hex_color(RED_GREEN_INSIDE.choose(rng).unwrap()),
            Some(IshiharaColor::Outside) => hex_color(RED_GREEN_OUTSIDE.choose(rng).unwrap()),
            None => hex_color("#ffffff"),
        }
        .unwrap();

        draw_filled_circle_mut(
            image,
            (self.center.x as i32, self.center.y as i32),
            self.radius as i32,
            Rgba([color.red, color.green, color.blue, 255]),
        );
    }
}

fn max_allowed_radius(
    candidate_point: &Point2D,
    circles: &[Circle],
    _rng: &mut rand::rngs::ThreadRng,
) -> Option<f64> {
    let mut curr_radius = Circle::MAX_RADIUS;
    for other in circles {
        let edge_distance = candidate_point.distance(&other.center) - other.radius;
        curr_radius = curr_radius.min(edge_distance - 1.0);
        if curr_radius < Circle::MIN_RADIUS {
            return None;
        }
    }
    Some(curr_radius)
}

fn render_text(text: &str) -> image::RgbaImage {
    let font_data = include_bytes!("../resources/fonts/Roboto-Regular.ttf");
    let font = Font::try_from_bytes(font_data as &[u8]).expect("Error constructing Font");
    let scale = Scale::uniform(256.0);
    let color = Color {
        red: 0,
        green: 0,
        blue: 0,
    }; // black
    let v_metrics = font.v_metrics(scale);

    // layout the glyphs in a line with 20 pixels padding
    let glyphs: Vec<_> = font
        .layout(text, scale, point(20.0, 20.0 + v_metrics.ascent))
        .collect();

    // work out the layout size
    let glyphs_height = (v_metrics.ascent - v_metrics.descent).ceil() as u32;
    let glyphs_width = {
        let min_x = glyphs
            .first()
            .map(|g| g.pixel_bounding_box().unwrap().min.x)
            .unwrap();
        let max_x = glyphs
            .last()
            .map(|g| g.pixel_bounding_box().unwrap().max.x)
            .unwrap();
        (max_x - min_x) as u32
    };

    // Create a new rgba image with some padding
    let mut image = DynamicImage::new_rgba8(glyphs_width + 40, glyphs_height + 40).to_rgba8();

    // Loop through the glyphs in the text, positing each one on a line
    for glyph in glyphs {
        if let Some(bounding_box) = glyph.pixel_bounding_box() {
            // Draw the glyph into the image per-pixel by using the draw closure
            glyph.draw(|x, y, v| {
                image.put_pixel(
                    // Offset the position by the glyph bounding box
                    x + bounding_box.min.x as u32,
                    y + bounding_box.min.y as u32,
                    // Turn the coverage into an alpha value
                    Rgba([color.red, color.green, color.blue, (v * 255.0) as u8]),
                )
            });
        }
    }
    image
}

fn main() {
    if let Some(text) = env::args().nth(1) {
        let mut rng = rand::thread_rng();
        let mut image = render_text(&text);

        let file_name = format!("{}.png", text);

        let (x, y) = image.dimensions();
        let mut circles = Circle::create_circles(x, y, &mut rng);
        circles.iter().for_each(|circle| println!("{}", circle));
        circles
            .iter_mut()
            .for_each(|circle| circle.assign_color(&image));
        draw_filled_rect_mut(
            &mut image,
            imageproc::rect::Rect::at(0, 0).of_size(x, y),
            Rgba([255, 255, 255, 255]),
        );
        circles
            .iter()
            .for_each(|circle| circle.draw(&mut image, &mut rng));
        // Save the image to a png file
        image.save(&file_name).unwrap();
        println!("Generated: {}", &file_name);
        std::process::exit(0);
    }
    std::process::exit(1);
}

use image::{self, ImageBuffer, Rgb};
use imageproc::drawing::draw_line_segment_mut;
use rand;
use std::f64::{self, consts::PI};

const MIN_INFILL_LINE_LENGTH: f64 = 5.0;

#[derive(Clone, Copy, Debug)]
struct Point2D {
    x: f64,
    y: f64,
}

#[derive(Debug)]
struct BoundingBox {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

fn line_intersection(p1: Point2D, p2: Point2D, p3: Point2D, p4: Point2D) -> Option<Point2D> {
    let denom = (p4.y - p3.y) * (p2.x - p1.x) - (p4.x - p3.x) * (p2.y - p1.y);
    if denom == 0.0 {
        return None;
    }
    let ua = ((p4.x - p3.x) * (p1.y - p3.y) - (p4.y - p3.y) * (p1.x - p3.x)) / denom;
    let ub = ((p2.x - p1.x) * (p1.y - p3.y) - (p2.y - p1.y) * (p1.x - p3.x)) / denom;
    if ua < 0.0 || ua > 1.0 || ub < 0.0 || ub > 1.0 {
        return None;
    }
    Some(Point2D {
        x: p1.x + ua * (p2.x - p1.x),
        y: p1.y + ua * (p2.y - p1.y),
    })
}

fn get_bounding_box(polygon: &[Point2D]) -> BoundingBox {
    let (min_x, max_x) = polygon
        .iter()
        .map(|p| p.x)
        .fold((f64::MAX, f64::MIN), |(min, max), x| {
            (min.min(x), max.max(x))
        });
    let (min_y, max_y) = polygon
        .iter()
        .map(|p| p.y)
        .fold((f64::MAX, f64::MIN), |(min, max), y| {
            (min.min(y), max.max(y))
        });
    BoundingBox {
        min_x,
        min_y,
        max_x,
        max_y,
    }
}

fn is_point_in_polygon(point: Point2D, polygon: &[Point2D]) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let j = (i + 1) % polygon.len();
        let xi = polygon[i].x;
        let yi = polygon[i].y;
        let xj = polygon[j].x;
        let yj = polygon[j].y;
        let intersect = ((yi > point.y) != (yj > point.y))
            && (point.x < (xj - xi) * (point.y - yi) / (yj - yi) + xi);
        if intersect {
            inside = !inside;
        }
    }
    inside
}

fn hatch_fill_polygon(polygon: &[Point2D], spacing: f64, angle: f64) -> Vec<(Point2D, Point2D)> {
    let bbox = get_bounding_box(polygon);
    let center_x = (bbox.min_x + bbox.max_x) / 2.0;
    let center_y = (bbox.min_y + bbox.max_y) / 2.0;

    let diagonal = ((bbox.max_x - bbox.min_x).powi(2) + (bbox.max_y - bbox.min_y).powi(2)).sqrt();

    let angle_rad = angle.to_radians();

    let num_lines = (diagonal / spacing).ceil() as i32 * 2;

    let mut hatch_lines = Vec::new();

    for i in -num_lines / 2..num_lines / 2 {
        let line_offset = i as f64 * spacing;

        let x1 = center_x - diagonal * angle_rad.cos() - line_offset * angle_rad.sin();
        let y1 = center_y - diagonal * angle_rad.sin() + line_offset * angle_rad.cos();
        let x2 = center_x + diagonal * angle_rad.cos() - line_offset * angle_rad.sin();
        let y2 = center_y + diagonal * angle_rad.sin() + line_offset * angle_rad.cos();

        let mut intersections = Vec::new();
        for j in 0..polygon.len() {
            let k = (j + 1) % polygon.len();
            if let Some(intersection) = line_intersection(
                Point2D { x: x1, y: y1 },
                Point2D { x: x2, y: y2 },
                polygon[j],
                polygon[k],
            ) {
                intersections.push(intersection);
            }
        }

        intersections.sort_by(|a, b| {
            let dist_a = (a.x - x1).powi(2) + (a.y - y1).powi(2);
            let dist_b = (b.x - x1).powi(2) + (b.y - y1).powi(2);
            dist_a.partial_cmp(&dist_b).unwrap()
        });

        for j in (0..intersections.len()).step_by(2) {
            if j + 1 < intersections.len() {
                hatch_lines.push((intersections[j], intersections[j + 1]));
            }
        }
    }

    // filter for lines which are longer than 2.0
    let filtered_lines: Vec<(Point2D, Point2D)> = hatch_lines
        .iter()
        .filter(|(p1, p2)| {
            let length = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt();
            length > MIN_INFILL_LINE_LENGTH
        })
        .cloned()
        .collect();
    filtered_lines
}

fn draw_hatched_polygon(
    polygon: &[Point2D],
    spacing: f64,
    angle: f64,
    cross_hatched: bool,
) -> Vec<(Point2D, Point2D)> {
    let mut lines = hatch_fill_polygon(polygon, spacing, angle);
    if cross_hatched {
        lines.extend(hatch_fill_polygon(polygon, spacing, angle + 90.0));
    }
    lines
}

fn generate_random_polygon(points: usize, width: f64, height: f64) -> Vec<Point2D> {
    use image::{ImageBuffer, Rgb};
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..points)
        .map(|_| Point2D {
            x: rng.gen_range(0.0..width),
            y: rng.gen_range(0.0..height),
        })
        .collect()
}

fn draw_polygon(polygon: &[Point2D], color: Rgb<u8>) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let bbox = get_bounding_box(polygon);
    let width = (bbox.max_x - bbox.min_x).ceil() as u32;
    let height = (bbox.max_y - bbox.min_y).ceil() as u32;

    let mut image = ImageBuffer::new(width, height);

    for i in 0..polygon.len() {
        let j = (i + 1) % polygon.len();
        let p1 = polygon[i];
        let p2 = polygon[j];
        draw_line_segment_mut(
            &mut image,
            ((p1.x - bbox.min_x) as f32, (p1.y - bbox.min_y) as f32),
            ((p2.x - bbox.min_x) as f32, (p2.y - bbox.min_y) as f32),
            color,
        );
    }

    image
}

fn draw_hatch_lines(
    hatch_lines: &[(Point2D, Point2D)],
    color: Rgb<u8>,
    image: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    bbox: &BoundingBox,
) {
    for (p1, p2) in hatch_lines {
        let x1 = (p1.x.round() - bbox.min_x) as f32;
        let y1 = (p1.y.round() - bbox.min_y) as f32;
        let x2 = (p2.x.round() - bbox.min_x) as f32;
        let y2 = (p2.y.round() - bbox.min_y) as f32;
        draw_line_segment_mut(image, (x1, y1), (x2, y2), color);
    }
}

fn angle_between_points(p1: Point2D, p2: Point2D) -> f64 {
    (p2.y - p1.y).atan2(p2.x - p1.x)
}

fn normalize_angle(angle: f64) -> f64 {
    (angle + PI) % PI
}

fn find_furthest_angle(polygon: &[Point2D]) -> f64 {
    let mut edge_angles: Vec<f64> = Vec::new();

    // Calculate angles of all edges
    for i in 0..polygon.len() {
        let j = (i + 1) % polygon.len();
        let angle = normalize_angle(angle_between_points(polygon[i], polygon[j]));
        edge_angles.push(angle);
    }

    let mut max_min_difference = 0.0;
    let mut furthest_angle = 0.0;

    // Sample angles and find the one with the maximum minimum difference from edge angles
    for i in 0..360 {
        let sample_angle = normalize_angle(i as f64 * PI / 360 as f64);
        let min_difference = edge_angles
            .iter()
            .map(|&edge_angle| {
                let diff = (sample_angle - edge_angle).abs();
                diff.min(PI - diff)
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        if min_difference > max_min_difference {
            max_min_difference = min_difference;
            furthest_angle = sample_angle;
        }
    }

    furthest_angle
}

fn main() {
    let polygon = generate_random_polygon(6, 200.0, 200.0);

    let furthest_angle = find_furthest_angle(&polygon).to_degrees();

    let hatch_lines = draw_hatched_polygon(&polygon, 10.0, furthest_angle, true);

    let mut image = draw_polygon(&polygon, Rgb([0, 255, 0]));
    draw_hatch_lines(
        &hatch_lines,
        Rgb([255, 0, 0]),
        &mut image,
        &get_bounding_box(&polygon),
    );

    image.save("output.png").unwrap();
}

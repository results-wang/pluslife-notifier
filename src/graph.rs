use std::path::Path;

use plotters::{
    chart::ChartBuilder,
    prelude::{BitMapBackend, Circle, EmptyElement, IntoDrawingArea},
    series::{LineSeries, PointSeries},
    style::{RGBAColor, RGBColor, ShapeStyle, WHITE},
};

use crate::{Error, messages::TestData};

pub const WIDTH: u32 = 800;
pub const HEIGHT: u32 = 600;

pub struct GraphData {
    pub min_time: f32,
    pub max_time: f32,
    pub min_value: i64,
    pub max_value: i64,
    pub lines: Vec<Line>,
}

impl GraphData {
    pub fn normalise_values_to_zero(&self) -> GraphData {
        let line_minima: Vec<_> = self
            .lines
            .iter()
            .map(|line| {
                line.points
                    .iter()
                    .map(|(_, value)| *value)
                    .min()
                    .unwrap_or_default()
            })
            .collect();

        let normalised_lines: Vec<Line> = self
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| Line {
                color: line.color,
                points: line
                    .points
                    .iter()
                    .map(|(time, value)| (*time, value - line_minima[index]))
                    .collect(),
            })
            .collect();

        GraphData {
            min_time: self.min_time,
            max_time: self.max_time,
            min_value: 0,
            max_value: normalised_lines
                .iter()
                .map(|line| {
                    line.points
                        .iter()
                        .map(|(_, value)| *value)
                        .max()
                        .unwrap_or_default()
                })
                .max()
                .unwrap_or_default(),
            lines: normalised_lines,
        }
    }

    pub fn plot_to_buffer(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0; (WIDTH * HEIGHT * 3) as usize];
        let backend = BitMapBackend::with_buffer(&mut buf, (WIDTH, HEIGHT));
        self.plot(backend)?;

        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, WIDTH, HEIGHT);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .expect("PNG configuration should be valid and writes should succeed")
            .write_image_data(&buf)
            .expect("PNG configuration should be valid and writes should succeed");
        Ok(out)
    }

    pub fn plot_to_file(&self, path: &Path) -> Result<(), Error> {
        let backend = BitMapBackend::new(path, (800, 600));
        self.plot(backend)
    }

    fn plot<'a>(&self, backend: BitMapBackend<'a>) -> Result<(), Error> {
        let root = backend.into_drawing_area();
        root.fill(&WHITE)?;
        let root = root.margin(10, 10, 10, 10);

        let mut chart = ChartBuilder::on(&root)
            // .caption("This is our first plot", ("sans-serif", 40).into_font())
            .x_label_area_size(20)
            .y_label_area_size(40)
            .build_cartesian_2d(
                (self.min_time)..(self.max_time),
                (self.min_value)..(i64::max(self.max_value * 2, 5000)),
            )?;

        chart
            .configure_mesh()
            .x_labels(((self.max_time - self.min_time) / 5f32) as usize)
            .disable_y_axis()
            .disable_y_mesh()
            .light_line_style(ShapeStyle {
                color: RGBAColor(255, 255, 255, 0.0),
                filled: false,
                stroke_width: 0,
            })
            .draw()?;

        for line in &self.lines {
            chart.draw_series(LineSeries::new(line.points.clone(), &line.color))?;

            chart.draw_series(PointSeries::of_element(
                line.points.clone(),
                2,
                &line.color,
                &|coord, size, style| {
                    EmptyElement::at(coord) + Circle::new((0, 0), size, style.filled())
                },
            ))?;
        }
        root.present()?;
        Ok(())
    }
}

pub struct Line {
    pub color: RGBColor,
    pub points: Vec<(f32, i64)>,
}

impl Line {
    pub fn new(color: RGBColor) -> Line {
        Line {
            color,
            points: Vec::new(),
        }
    }
}

impl TestData {
    pub fn to_graph(&self) -> Result<GraphData, Error> {
        let mut min_time = f32::MAX;
        let mut max_time = f32::MIN;
        let mut min_value = i64::MAX;
        let mut max_value = i64::MIN;
        let mut lines = vec![
            Line::new(RGBColor(166, 206, 227)),
            Line::new(RGBColor(32, 120, 180)),
            Line::new(RGBColor(178, 223, 138)),
            Line::new(RGBColor(52, 160, 45)),
            Line::new(RGBColor(166, 206, 227)),
            Line::new(RGBColor(252, 154, 154)),
            Line::new(RGBColor(254, 192, 112)),
        ];

        for sample in &self.samples {
            let time_minutes = sample.sampling_time as f32 / 600f32;
            min_time = f32::min(min_time, time_minutes);
            max_time = f32::max(max_time, time_minutes);
            min_value = i64::min(min_value, sample.first_channel_result);
            max_value = i64::max(max_value, sample.first_channel_result);
            if sample.starting_channel >= lines.len() {
                return Err(Error::TooManyChannels(sample.starting_channel));
            }
            lines[sample.starting_channel]
                .points
                .push((time_minutes, sample.first_channel_result));
        }
        Ok(GraphData {
            min_time,
            max_time,
            min_value,
            max_value,
            lines,
        })
    }
}

use crate::color::base16::PaletteBackend;
use colorsys::Rgb;
use image::{imageops::FilterType, RgbImage};
use palette::IntoColor;
use palette::Oklab;

pub struct WalBackend {
    pub clusters: usize,
    pub resize_width: u32,
    pub sample_step: u32,
    pub iterations: usize,
}

impl Default for WalBackend {
    fn default() -> Self {
        Self {
            clusters: 16,
            resize_width: 300,
            sample_step: 2,
            iterations: 8,
        }
    }
}

impl PaletteBackend for WalBackend {
    fn extract(&self, image: &RgbImage) -> Vec<Rgb> {
        let resized = resize_image(image, self.resize_width);
        let samples = sample_pixels(&resized, self.sample_step);

        if samples.is_empty() {
            return Vec::new();
        }

        let centroids = kmeans(&samples, self.clusters, self.iterations);

        let mut colors: Vec<Rgb> = centroids
            .into_iter()
            .map(|c| Rgb::new(c[0].into(), c[1].into(), c[2].into(), None))
            .collect();

        colors.sort_by(|a, b| {
            let sa = score_colorsys(a);
            let sb = score_colorsys(b);
            sb.partial_cmp(&sa).unwrap()
        });

        colors
    }
}

fn resize_image(image: &RgbImage, target_width: u32) -> RgbImage {
    let (w, h) = image.dimensions();

    if w <= target_width {
        return image.clone();
    }

    let scale = target_width as f32 / w as f32;
    let target_height = (h as f32 * scale).round().max(1.0) as u32;
    image::imageops::resize(image, target_width, target_height, FilterType::Triangle)
}

fn sample_pixels(image: &RgbImage, step: u32) -> Vec<[f32; 3]> {
    let mut out = Vec::new();

    for y in (0..image.height()).step_by(step as usize) {
        for x in (0..image.width()).step_by(step as usize) {
            let p = image.get_pixel(x, y);
            out.push([p[0] as f32, p[1] as f32, p[2] as f32]);
        }
    }

    out
}

fn kmeans(samples: &[[f32; 3]], k: usize, iterations: usize) -> Vec<[f32; 3]> {
    let cluster_count = k.min(samples.len());
    if cluster_count == 0 {
        return Vec::new();
    }

    let mut centroids = if cluster_count == 1 {
        vec![samples[0]]
    } else {
        (0..cluster_count)
            .map(|index| {
                let sample_index = index * (samples.len() - 1) / (cluster_count - 1);
                samples[sample_index]
            })
            .collect::<Vec<_>>()
    };

    for _ in 0..iterations {
        let mut buckets: Vec<Vec<[f32; 3]>> = vec![Vec::new(); cluster_count];

        for sample in samples {
            let (best, _) = centroids
                .iter()
                .enumerate()
                .map(|(index, centroid)| (index, color_distance(*sample, *centroid)))
                .min_by(|(_, left), (_, right)| left.total_cmp(right))
                .expect("cluster_count is non-zero");
            buckets[best].push(*sample);
        }

        for (index, bucket) in buckets.iter().enumerate() {
            if bucket.is_empty() {
                centroids[index] = samples
                    .iter()
                    .copied()
                    .max_by(|left, right| {
                        nearest_centroid_distance(*left, &centroids)
                            .total_cmp(&nearest_centroid_distance(*right, &centroids))
                    })
                    .expect("samples is non-empty");
                continue;
            }

            let mut sum = [0.0; 3];
            for pixel in bucket {
                sum[0] += pixel[0];
                sum[1] += pixel[1];
                sum[2] += pixel[2];
            }
            centroids[index] = [
                sum[0] / bucket.len() as f32,
                sum[1] / bucket.len() as f32,
                sum[2] / bucket.len() as f32,
            ];
        }
    }

    centroids
}

fn color_distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    (left[0] - right[0]).powi(2)
        + (left[1] - right[1]).powi(2)
        + (left[2] - right[2]).powi(2)
}

fn nearest_centroid_distance(sample: [f32; 3], centroids: &[[f32; 3]]) -> f32 {
    centroids
        .iter()
        .map(|centroid| color_distance(sample, *centroid))
        .fold(f32::MAX, f32::min)
}

fn score_colorsys(c: &Rgb) -> f32 {
    let linear = [
        c.red() as f32 / 255.0,
        c.green() as f32 / 255.0,
        c.blue() as f32 / 255.0,
    ];

    let srgb = palette::Srgb::new(linear[0], linear[1], linear[2]);
    let lab: Oklab = srgb.into_linear().into_color();

    let lightness = lab.l; // 0–1
    let chroma = (lab.a * lab.a + lab.b * lab.b).sqrt();

    chroma * 2.0 + (0.5 - lightness).abs()
}

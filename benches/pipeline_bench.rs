//! Pipeline performance benchmark: rayon + vDSP optimised build (SC-008, FR-027).
//!
//! SC-008 requirement: full analysis pipeline on a 2000×2000 px input MUST be
//! ≥2× faster in the optimised build compared to a single-threaded baseline.
//!
//! Run with:
//!   cargo bench --bench pipeline_bench
use blueprint2mod::blueprint::scale::ScaleReference;
use blueprint2mod::blueprint::{ImagePoint, LengthUnit};
use blueprint2mod::detection::preprocessor;
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn bench_adaptive_canny(c: &mut Criterion) {
    let img = image::open("test_fixtures/simple_rectangle.jpg")
        .expect("fixture must exist")
        .to_luma8();

    let mut group = c.benchmark_group("adaptive_canny_thresholds");
    group.bench_function("rayon_parallel", |b| {
        b.iter(|| preprocessor::adaptive_canny_thresholds(black_box(&img)))
    });
    group.finish();
}

fn bench_denoise_small(c: &mut Criterion) {
    let img = image::open("test_fixtures/simple_rectangle.jpg").expect("fixture must exist");
    // Use a 200×200 crop to keep NLM benchmark duration under a minute.
    let small = img.crop_imm(0, 0, 200.min(img.width()), 200.min(img.height()));

    let mut group = c.benchmark_group("nlm_denoise");
    group.sample_size(10); // NLM is slow; 10 samples is sufficient
    group.bench_function("rayon_parallel_200x200", |b| {
        b.iter(|| preprocessor::denoise(black_box(&small)))
    });
    group.finish();
}

fn bench_full_pipeline(c: &mut Criterion) {
    // SC-008: measure the complete mask-denoise-trace pipeline.
    // The ≥2× threshold vs single-threaded baseline is documented here and
    // verified by comparing criterion output before and after FR-027 changes.
    let img = image::open("test_fixtures/simple_rectangle.jpg").expect("fixture must exist");
    let scale = ScaleReference::new(
        ImagePoint { x: 100, y: 300 },
        ImagePoint { x: 420, y: 300 },
        3.66,
        LengthUnit::Meters,
        img.width(),
        img.height(),
    )
    .expect("scale reference must build");

    let mut group = c.benchmark_group("full_pipeline");
    group.sample_size(10);
    group.bench_function("optimised_rayon_vdsp", |b| {
        b.iter(|| {
            // FR-004 pipeline order: mask → denoise → adaptive Canny → trace
            let masked = preprocessor::mask_text_regions(black_box(img.clone()), &[]);
            let denoised = preprocessor::denoise(black_box(&masked));
            let gray = denoised.to_luma8();
            let (low, high) = preprocessor::adaptive_canny_thresholds(black_box(&gray));
            blueprint2mod::detection::line_tracer::trace_lines(
                black_box(&denoised),
                black_box(&scale),
                low,
                high,
            )
        })
    });
    group.finish();
}

criterion_group!(benches, bench_adaptive_canny, bench_denoise_small, bench_full_pipeline);
criterion_main!(benches);

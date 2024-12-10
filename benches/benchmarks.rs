use criterion::{criterion_group, criterion_main, Criterion};
use sentinel2array::Raster;
use tokio::runtime::Runtime;

//const N_BLOCKS: usize = 1;
const SIZE: (usize, usize) = (2048, 2048);

fn bench_read_bands(c: &mut Criterion) {
    let raster =
        Raster::new("data/S2A_MSIL2A_20241017T102021_N0511_R065_T32UQD_20241017T143350.SAFE.zip")
            .unwrap();
    c.bench_function("read_bands", |b| {
        b.iter(|| raster.get_array(vec!["B4", "B3", "B2"], (0, 0), SIZE))
    });
}

fn bench_read_bands_async(c: &mut Criterion) {
    let raster =
        Raster::new("data/S2A_MSIL2A_20241017T102021_N0511_R065_T32UQD_20241017T143350.SAFE.zip")
            .unwrap();
    c.bench_function("read_bands_async", |b| {
        b.to_async(Runtime::new().unwrap())
            .iter(|| raster.get_array_async(vec!["B4", "B3", "B2"], (0, 0), SIZE))
    });
}

criterion_group!(benches, bench_read_bands, bench_read_bands_async);
criterion_main!(benches);

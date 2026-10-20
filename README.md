# Lipatin

Alat pola kemasan untuk percetakan, dibuat untuk tim XGraphics dan d'art box.

- **Box**: dieline kotak lipat (reverse tuck, straight tuck, tray, mailer, tutup-alas, laci, hexagonal) dan hardbox (tutup-alas, box magnet buku, laci, box leher). Pola board, kertas pembungkus, dan kertas dalam skala 1:1, pratinjau rakit 3D, hitung isi per plano.
- **Layout cetak**: satu desain banyak copy, gabung banyak order, susun halaman buku, banner di roll, akrilik untuk laser.
- **Estimasi harga**: tempel spek dari chat, harga keluar dari daftar harga sendiri.
- **Ekspor**: SVG, PDF untuk Illustrator (warna spot CutContour, Crease, Bleed), PDF layout siap cetak, SVG dan DXF untuk laser.

Semua berjalan di browser, tidak ada server. Buka `index.html`, atau lewat GitHub Pages.

## Catatan

Daftar harga di tab Estimasi harga tersimpan di browser masing-masing, tidak ikut ke repo ini. Untuk menyamakan harga satu tim, pakai tombol **Simpan daftar harga ke file** lalu kirim file itu, dan teman satu tim memakai **Muat dari file**.

## Pengembangan

- `node tests/golden.mjs` membandingkan hasil mesin pola (11 model × 3 ukuran) dengan `tests/golden/engine.json`. Jalankan setelah mengubah kode model; kalau perubahannya memang disengaja, perbarui dengan `--update`.
- `nest/` adalah inti nesting di Rust: menghitung berapa pola yang muat di plano kalau bentuk dieline yang sebenarnya saling dikunci. Tab **Plano & biaya** memakainya lewat WebAssembly (`lipatin_nest_bg.wasm`, `lipatin_nest.js`, `nest-worker.js`) di Web Worker, dimuat saat tab dibuka. Hitungan kotak pembatas (`impose()`) tampil lebih dulu dan tetap dipakai kalau WebAssembly gagal dimuat atau susunan dikunci tidak menambah isi.
  - Tes: di `nest/`, `cargo test`; dari root, `node tests/nest-wasm.mjs` menjalankan file `.wasm` yang di-commit pada semua model kotak lipat.
  - Bandingkan dengan kotak pembatas: `node tests/export-outline.mjs 90 60 30 > nest/jobs-90x60x30.json`, lalu di `nest/`: `cargo run --release -- jobs-90x60x30.json out`. Gambar susunannya ada di `nest/out/`.
  - Build ulang WebAssembly setelah mengubah `nest/src/lib.rs` (toolchain GNU, target `wasm32-unknown-unknown`, `wasm-bindgen` CLI versi 0.2.129, sama dengan `Cargo.toml`), lalu commit dua file hasilnya:
    ```
    cd nest
    cargo build --release --lib --target wasm32-unknown-unknown
    wasm-bindgen --target web --no-typescript --out-dir .. target/wasm32-unknown-unknown/release/lipatin_nest.wasm
    ```

# miniquad 0.4.8 (alice-web yaması)

crates.io'daki `miniquad 0.4.8`in birebir kopyası; `port/lookingglass/Cargo.toml` içindeki
`[patch.crates-io]` ile kullanılır. Lisans: MIT / Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`).

Neden: tarayıcıda her GL çağrısı wasm'dan JavaScript'e geçer. macroquad her çizimden önce tüm
durumu (framebuffer, viewport, scissor, program, derinlik, ~40 uniform) yeniden gönderiyor;
hedge3'te bu kare başına ~41.000 uniform ve binlerce durum çağrısı demekti.

Değişiklikler (`// lgweb` ile işaretli):

- `src/graphics/gl/dedup.rs` (yeni): `glBindFramebuffer`, `glViewport`, `glScissor`, `glUseProgram`,
  `glEnable`/`glDisable` (scissor ve derinlik testi), `glDepthFunc`, `glFrontFace` son gönderilen
  değerle aynıysa atlanır. Framebuffer ya da program silinince önbellek unutulur.
- `src/graphics/gl.rs`: `apply_uniforms_from_bytes` her program için son gönderilen uniform
  baytlarını tutar, yalnızca değişenleri yükler. Sampler birimleri (görüntü n → birim n) program
  başına bir kez ayarlanır.

Sonuç (headless Chrome, hedge3, kilitsiz, thin LTO): kare başına uniform çağrısı 41.224 → 66;
`draw_batch` değişikliğiyle birlikte ~39 → ~49 fps.

Güncellemek için: yeni sürümü buraya kopyalayın ve `lgweb` işaretli değişiklikleri yeniden uygulayın.

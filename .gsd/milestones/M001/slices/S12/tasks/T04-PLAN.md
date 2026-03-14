# T04: Cloudflare Pages Deployment

**Slice:** S12 — **Milestone:** M001

## Description

Set up Cloudflare Pages deployment with GitHub Actions, proper WASM headers, and CSP configuration.

Purpose: WEB-03 (HTTPS), WEB-09 (global CDN). Production deployment with correct headers for WASM execution.
Output: GitHub Actions workflow + Cloudflare Pages config files.

## Must-Haves

- [ ] "Web application is served over HTTPS via CDN"
- [ ] "WASM files served with correct Content-Type and compression"
- [ ] "CSP header includes wasm-unsafe-eval for WASM execution"
- [ ] "Deployment triggers automatically on push to main"

## Files

- `.github/workflows/deploy.yml`
- `viewer/public/_headers`
- `viewer/public/_redirects`

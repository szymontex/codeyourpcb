# T02: File System Access API

**Slice:** S12 — **Milestone:** M001

## Description

Implement File System Access API for opening and saving local files, with fallback for Firefox and older browsers.

Purpose: WEB-05 and WEB-06 require users to open/save local files without server involvement. The existing file-picker.ts uses basic input element — this adds native file handle support for save-in-place.
Output: file-access.ts module with open/save functions, integrated into main.ts.

## Must-Haves

- [ ] "User can open local .cypcb files via File System Access API in Chrome/Edge/Safari"
- [ ] "User can open files in Firefox via fallback input element"
- [ ] "User can save files back to disk without save-as dialog (when handle available)"
- [ ] "User can save via download fallback when File System Access API unavailable"

## Files

- `viewer/src/file-access.ts`
- `viewer/src/file-picker.ts`
- `viewer/src/main.ts`

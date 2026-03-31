# UI Improvement Changelog (10 Iterations)

## Iteration 1: Toolbar background
- Added semi-transparent background to the top toolbar
- Creates clear visual separation between toolbar and file panels
- Files: `src/ui/app.rs` (toolbar wrapped in styled container)

## Iteration 2: Fkey bar styling
- Added background to function key bar at bottom
- Increased button padding (2,8 -> 4,12) and font size (11 -> 12)
- More spacing between buttons (2 -> 4)
- Files: `src/ui/app.rs` (make_fkey_bar closure)

## Iteration 3: Status bar background
- Added subtle background to the status bar
- Increased font size (11 -> 12) and padding (2,12 -> 4,12)
- Files: `src/ui/app.rs` (status_bar container)

## Iteration 4: Transfer buttons polish
- Added triangle arrow icons (Upload ▶ / ◀ Download)
- Increased button padding (6 -> 6,14) and spacing (8 -> 12)
- More container padding around transfer controls
- Files: `src/ui/app.rs` (upload_btn/download_btn)

## Iteration 5: Panel breadcrumb improvements
- Replaced "<" back button with ◀ unicode arrow
- Replaced "F5" refresh button with ↻ unicode symbol
- Tighter breadcrumb spacing for more path visibility
- Files: `src/ui/file_panel.rs` (breadcrumbs section)

## Iteration 6: File row readability
- Added vertical alignment (align_y Center) to row contents
- Adjusted padding from uniform 4 to asymmetric [3,6]
- Slightly smaller permissions/modified text (13 -> 11) to reduce noise
- Files: `src/ui/file_panel.rs` (file row map closure)

## Iteration 7: Panel spacing
- Increased column spacing inside panels (4 -> 6)
- Files: `src/ui/file_panel.rs` (panel column)

## Iteration 8: Connect form card styling
- Added card container with border and semi-transparent background
- Added "Open SFTP" title above "Connect to Server" subtitle
- Port and Username fields now on the same row
- Increased form spacing (12 -> 14) and max width (400 -> 420)
- Larger button padding with proper text sizing
- Files: `src/ui/connect_dialog.rs`

## Iteration 9: Window title
- Changed window title from "EuroSFTP" to "Open SFTP"
- Files: `src/main.rs`

## Iteration 10: Log panel and progress bar polish
- Log panel: added background, border, more padding, increased height (100 -> 120px)
- Log panel: brighter success color (0.3 -> 0.5 green channel)
- Progress bar: increased height (8 -> 12px), better padding
- Added `demo` feature flag for screenshot builds (cfg-gated mock data)
- Files: `src/ui/app.rs`, `Cargo.toml`

## Stats
- Tests: 183/183 passing (0 regressions across all iterations)
- Files modified: 4 (app.rs, file_panel.rs, connect_dialog.rs, main.rs)
- Screenshots: before/after in `screenshots/`

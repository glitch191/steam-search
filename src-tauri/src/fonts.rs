//! Installed font families, read through DirectWrite for the font picker in the settings window.

use windows::core::w;
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteLocalizedStrings, DWRITE_FACTORY_TYPE_SHARED,
};

/// Family names of the installed fonts, sorted case-insensitively, without vertical (`@`) variants.
pub fn installed_families() -> Vec<String> {
    let mut names = match unsafe { read_families() } {
        Ok(names) => names,
        Err(e) => {
            crate::config::log(&format!("cannot list installed fonts: {e}"));
            Vec::new()
        }
    };
    names.retain(|n| !n.is_empty() && !n.starts_with('@'));
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}

unsafe fn read_families() -> windows::core::Result<Vec<String>> {
    let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
    let mut collection: Option<IDWriteFontCollection> = None;
    factory.GetSystemFontCollection(&mut collection, false)?;
    let Some(collection) = collection else { return Ok(Vec::new()) };
    let mut names = Vec::new();
    for i in 0..collection.GetFontFamilyCount() {
        if let Ok(family) = collection.GetFontFamily(i) {
            if let Ok(strings) = family.GetFamilyNames() {
                names.push(english_name(&strings)?);
            }
        }
    }
    Ok(names)
}

/// The English family name, which the web view matches in CSS, or the first one available.
unsafe fn english_name(strings: &IDWriteLocalizedStrings) -> windows::core::Result<String> {
    let mut index = 0;
    let mut exists = windows::core::BOOL(0);
    strings.FindLocaleName(w!("en-us"), &mut index, &mut exists)?;
    if !exists.as_bool() {
        index = 0;
    }
    let len = strings.GetStringLength(index)? as usize;
    let mut buffer = vec![0u16; len + 1];
    strings.GetString(index, &mut buffer)?;
    Ok(String::from_utf16_lossy(&buffer[..len]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn lists_common_windows_fonts() {
        let families = super::installed_families();
        assert!(families.iter().any(|f| f == "Segoe UI"), "Segoe UI not found in {} families", families.len());
    }
}

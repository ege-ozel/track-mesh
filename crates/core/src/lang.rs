//! The Desktop window's language, also used by its native dialogs and the DAW plugin.
use std::sync::atomic::{AtomicBool, Ordering};

static TURKISH: AtomicBool = AtomicBool::new(false);

pub fn is_turkish() -> bool {
    TURKISH.load(Ordering::Relaxed)
}

pub fn set_turkish(turkish: bool) {
    TURKISH.store(turkish, Ordering::Relaxed);
}

pub fn tr<'a>(en: &'a str, turkish: &'a str) -> &'a str {
    if is_turkish() {
        turkish
    } else {
        en
    }
}

/// A name with the Turkish genitive suffix by vowel harmony (Ege'nin, Burak'ın), as the window's `named(name, 'gen')`.
pub fn genitive(name: &str) -> String {
    let lower = name
        .to_lowercase()
        .replace('â', "a")
        .replace('î', "i")
        .replace('û', "u");
    let is_vowel = |ch: char| "aeıioöuü".contains(ch);
    let last = lower.chars().rev().find(|ch| is_vowel(*ch)).unwrap_or('e');
    let ends_vowel = lower.chars().last().is_some_and(is_vowel);
    let four = match last {
        'a' | 'ı' => 'ı',
        'e' | 'i' => 'i',
        'o' | 'u' => 'u',
        _ => 'ü',
    };
    format!("{name}'{}{four}n", if ends_vowel { "n" } else { "" })
}

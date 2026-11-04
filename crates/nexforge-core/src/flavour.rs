//! Game flavours. Every host maps its own labels to [`Flavour`] inside its own
//! source module; this file only knows about the game client itself.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Flavour {
    Forever,
    ClassicEra,
    Anniversary,
    /// Reserved: not supported in v1, but kept so state files stay forward compatible.
    Retail,
}

impl Flavour {
    pub const ALL: [Flavour; 4] = [
        Flavour::Forever,
        Flavour::ClassicEra,
        Flavour::Anniversary,
        Flavour::Retail,
    ];

    /// Stable machine name, as written to `addons.toml`.
    pub fn as_str(self) -> &'static str {
        match self {
            Flavour::Forever => "forever",
            Flavour::ClassicEra => "classic-era",
            Flavour::Anniversary => "anniversary",
            Flavour::Retail => "retail",
        }
    }

    /// Name shown to players.
    pub fn label(self) -> &'static str {
        match self {
            Flavour::Forever => "WoW Forever",
            Flavour::ClassicEra => "Classic Era",
            Flavour::Anniversary => "Anniversary",
            Flavour::Retail => "Retail",
        }
    }

    /// Short label used in sentences such as "No Forever build listed".
    pub fn short_label(self) -> &'static str {
        match self {
            Flavour::Forever => "Forever",
            Flavour::ClassicEra => "Classic Era",
            Flavour::Anniversary => "Anniversary",
            Flavour::Retail => "Retail",
        }
    }

    /// Map a client folder name (`_forever_`, `_classic_era_` …) to a flavour.
    pub fn from_folder_name(name: &str) -> Option<Flavour> {
        match name.to_ascii_lowercase().as_str() {
            "_forever_" => Some(Flavour::Forever),
            "_classic_era_" => Some(Flavour::ClassicEra),
            "_anniversary_" => Some(Flavour::Anniversary),
            "_retail_" => Some(Flavour::Retail),
            _ => None,
        }
    }

    /// Map the product line Blizzard writes into `.flavor.info`.
    ///
    /// Observed on 2026-11-04: `_forever_` → `wowf`, `_classic_era_` → `wow_classic_era`,
    /// `_anniversary_` → `wow_anniversary`, `_retail_` → `wow`. `_classic_` (`wow_classic`,
    /// the progression realm client) is not a v1 flavour.
    pub fn from_product(product: &str) -> Option<Flavour> {
        match product.trim().to_ascii_lowercase().as_str() {
            "wowf" | "wow_forever" => Some(Flavour::Forever),
            "wow_classic_era" => Some(Flavour::ClassicEra),
            "wow_anniversary" => Some(Flavour::Anniversary),
            "wow" => Some(Flavour::Retail),
            _ => None,
        }
    }

    /// Infer a flavour from a TOC `## Interface` number.
    /// 16xxx Forever · 11xxx Classic Era · 2xxxx TBC/Anniversary · 12xxxx Retail.
    pub fn from_interface(interface: u32) -> Option<Flavour> {
        match interface {
            16000..=16999 => Some(Flavour::Forever),
            11000..=11999 => Some(Flavour::ClassicEra),
            20000..=29999 => Some(Flavour::Anniversary),
            120000..=129999 => Some(Flavour::Retail),
            _ => None,
        }
    }

    /// Flavour-specific TOC suffixes, most specific first. The empty string is
    /// the plain `<Folder>.toc`.
    pub fn toc_suffixes(self) -> &'static [&'static str] {
        match self {
            Flavour::Forever => &["Forever", "Camelot", ""],
            Flavour::ClassicEra => &["Vanilla", "Classic", ""],
            Flavour::Anniversary => &["TBC", "BCC", "Vanilla", ""],
            Flavour::Retail => &["Mainline", ""],
        }
    }
}

impl fmt::Display for Flavour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Flavour {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let norm = s.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        match norm.as_str() {
            "forever" | "wowf" | "camelot" => Ok(Flavour::Forever),
            "classic-era" | "era" | "vanilla" => Ok(Flavour::ClassicEra),
            "anniversary" | "tbc" | "bcc" => Ok(Flavour::Anniversary),
            "retail" | "mainline" => Ok(Flavour::Retail),
            _ => Err(format!(
                "\"{s}\" is not a flavour. Use forever, classic-era or anniversary."
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_and_product_names() {
        assert_eq!(
            Flavour::from_folder_name("_forever_"),
            Some(Flavour::Forever)
        );
        assert_eq!(Flavour::from_folder_name("_classic_"), None);
        assert_eq!(Flavour::from_product("wowf"), Some(Flavour::Forever));
        assert_eq!(
            Flavour::from_product("wow_classic_era"),
            Some(Flavour::ClassicEra)
        );
        assert_eq!(Flavour::from_product("wow_classic"), None);
    }

    #[test]
    fn interface_ranges() {
        assert_eq!(Flavour::from_interface(16001), Some(Flavour::Forever));
        assert_eq!(Flavour::from_interface(11508), Some(Flavour::ClassicEra));
        assert_eq!(Flavour::from_interface(20505), Some(Flavour::Anniversary));
        assert_eq!(Flavour::from_interface(50504), None);
        assert_eq!(Flavour::from_interface(120100), Some(Flavour::Retail));
    }

    #[test]
    fn parse_round_trip() {
        for f in Flavour::ALL {
            assert_eq!(f.as_str().parse::<Flavour>().unwrap(), f);
        }
        assert_eq!(
            "classic_era".parse::<Flavour>().unwrap(),
            Flavour::ClassicEra
        );
    }
}

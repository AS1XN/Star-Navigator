//! Constellation and Greek letter tables, and the text normalization used by search.

use std::collections::HashMap;
use std::sync::LazyLock;

pub struct Constellation {
    pub abbr: &'static str,
    pub name: &'static str,
    pub genitive: &'static str,
}

const fn c(abbr: &'static str, name: &'static str, genitive: &'static str) -> Constellation {
    Constellation { abbr, name, genitive }
}

pub const CONSTELLATIONS: [Constellation; 88] = [
    c("And", "Andromeda", "Andromedae"),
    c("Ant", "Antlia", "Antliae"),
    c("Aps", "Apus", "Apodis"),
    c("Aqr", "Aquarius", "Aquarii"),
    c("Aql", "Aquila", "Aquilae"),
    c("Ara", "Ara", "Arae"),
    c("Ari", "Aries", "Arietis"),
    c("Aur", "Auriga", "Aurigae"),
    c("Boo", "Bootes", "Bootis"),
    c("Cae", "Caelum", "Caeli"),
    c("Cam", "Camelopardalis", "Camelopardalis"),
    c("Cnc", "Cancer", "Cancri"),
    c("CVn", "Canes Venatici", "Canum Venaticorum"),
    c("CMa", "Canis Major", "Canis Majoris"),
    c("CMi", "Canis Minor", "Canis Minoris"),
    c("Cap", "Capricornus", "Capricorni"),
    c("Car", "Carina", "Carinae"),
    c("Cas", "Cassiopeia", "Cassiopeiae"),
    c("Cen", "Centaurus", "Centauri"),
    c("Cep", "Cepheus", "Cephei"),
    c("Cet", "Cetus", "Ceti"),
    c("Cha", "Chamaeleon", "Chamaeleontis"),
    c("Cir", "Circinus", "Circini"),
    c("Col", "Columba", "Columbae"),
    c("Com", "Coma Berenices", "Comae Berenices"),
    c("CrA", "Corona Australis", "Coronae Australis"),
    c("CrB", "Corona Borealis", "Coronae Borealis"),
    c("Crv", "Corvus", "Corvi"),
    c("Crt", "Crater", "Crateris"),
    c("Cru", "Crux", "Crucis"),
    c("Cyg", "Cygnus", "Cygni"),
    c("Del", "Delphinus", "Delphini"),
    c("Dor", "Dorado", "Doradus"),
    c("Dra", "Draco", "Draconis"),
    c("Equ", "Equuleus", "Equulei"),
    c("Eri", "Eridanus", "Eridani"),
    c("For", "Fornax", "Fornacis"),
    c("Gem", "Gemini", "Geminorum"),
    c("Gru", "Grus", "Gruis"),
    c("Her", "Hercules", "Herculis"),
    c("Hor", "Horologium", "Horologii"),
    c("Hya", "Hydra", "Hydrae"),
    c("Hyi", "Hydrus", "Hydri"),
    c("Ind", "Indus", "Indi"),
    c("Lac", "Lacerta", "Lacertae"),
    c("Leo", "Leo", "Leonis"),
    c("LMi", "Leo Minor", "Leonis Minoris"),
    c("Lep", "Lepus", "Leporis"),
    c("Lib", "Libra", "Librae"),
    c("Lup", "Lupus", "Lupi"),
    c("Lyn", "Lynx", "Lyncis"),
    c("Lyr", "Lyra", "Lyrae"),
    c("Men", "Mensa", "Mensae"),
    c("Mic", "Microscopium", "Microscopii"),
    c("Mon", "Monoceros", "Monocerotis"),
    c("Mus", "Musca", "Muscae"),
    c("Nor", "Norma", "Normae"),
    c("Oct", "Octans", "Octantis"),
    c("Oph", "Ophiuchus", "Ophiuchi"),
    c("Ori", "Orion", "Orionis"),
    c("Pav", "Pavo", "Pavonis"),
    c("Peg", "Pegasus", "Pegasi"),
    c("Per", "Perseus", "Persei"),
    c("Phe", "Phoenix", "Phoenicis"),
    c("Pic", "Pictor", "Pictoris"),
    c("Psc", "Pisces", "Piscium"),
    c("PsA", "Piscis Austrinus", "Piscis Austrini"),
    c("Pup", "Puppis", "Puppis"),
    c("Pyx", "Pyxis", "Pyxidis"),
    c("Ret", "Reticulum", "Reticuli"),
    c("Sge", "Sagitta", "Sagittae"),
    c("Sgr", "Sagittarius", "Sagittarii"),
    c("Sco", "Scorpius", "Scorpii"),
    c("Scl", "Sculptor", "Sculptoris"),
    c("Sct", "Scutum", "Scuti"),
    c("Ser", "Serpens", "Serpentis"),
    c("Sex", "Sextans", "Sextantis"),
    c("Tau", "Taurus", "Tauri"),
    c("Tel", "Telescopium", "Telescopii"),
    c("Tri", "Triangulum", "Trianguli"),
    c("TrA", "Triangulum Australe", "Trianguli Australis"),
    c("Tuc", "Tucana", "Tucanae"),
    c("UMa", "Ursa Major", "Ursae Majoris"),
    c("UMi", "Ursa Minor", "Ursae Minoris"),
    c("Vel", "Vela", "Velorum"),
    c("Vir", "Virgo", "Virginis"),
    c("Vol", "Volans", "Volantis"),
    c("Vul", "Vulpecula", "Vulpeculae"),
];

/// Index into [`CONSTELLATIONS`] for an IAU abbreviation (case-insensitive).
pub fn constellation_index(abbr: &str) -> Option<u8> {
    CONSTELLATIONS.iter().position(|c| c.abbr.eq_ignore_ascii_case(abbr)).map(|i| i as u8)
}

/// (name, catalog abbreviation, symbol)
pub const GREEK: [(&str, &str, char); 24] = [
    ("alpha", "alp", 'α'),
    ("beta", "bet", 'β'),
    ("gamma", "gam", 'γ'),
    ("delta", "del", 'δ'),
    ("epsilon", "eps", 'ε'),
    ("zeta", "zet", 'ζ'),
    ("eta", "eta", 'η'),
    ("theta", "the", 'θ'),
    ("iota", "iot", 'ι'),
    ("kappa", "kap", 'κ'),
    ("lambda", "lam", 'λ'),
    ("mu", "mu", 'μ'),
    ("nu", "nu", 'ν'),
    ("xi", "xi", 'ξ'),
    ("omicron", "omi", 'ο'),
    ("pi", "pi", 'π'),
    ("rho", "rho", 'ρ'),
    ("sigma", "sig", 'σ'),
    ("tau", "tau", 'τ'),
    ("upsilon", "ups", 'υ'),
    ("phi", "phi", 'φ'),
    ("chi", "chi", 'χ'),
    ("psi", "psi", 'ψ'),
    ("omega", "ome", 'ω'),
];

fn greek_abbr(token: &str) -> Option<&'static str> {
    GREEK.iter().find(|(name, abbr, _)| token == *name || token == *abbr).map(|g| g.1)
}

/// Full Greek letter name for a catalog abbreviation like "Alp" or "Alp-1".
pub fn greek_name(bayer: &str) -> Option<String> {
    let (letter, suffix) = bayer.split_once('-').unwrap_or((bayer, ""));
    let (name, _, _) = GREEK.iter().find(|g| g.1.eq_ignore_ascii_case(letter))?;
    let mut s = String::with_capacity(name.len() + suffix.len() + 1);
    s.push_str(&name[..1].to_ascii_uppercase());
    s.push_str(&name[1..]);
    if !suffix.is_empty() {
        s.push('-');
        s.push_str(suffix);
    }
    Some(s)
}

/// Multi-word constellation names as (" canis majoris ", " cma "), longest first so
/// "triangulum australe" wins over "triangulum".
static PHRASES: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    let mut phrases: Vec<(String, String)> = CONSTELLATIONS
        .iter()
        .flat_map(|c| [c.name, c.genitive].map(|p| (p, c.abbr)))
        .filter(|(p, _)| p.contains(' '))
        .map(|(p, a)| (format!(" {} ", p.to_lowercase()), format!(" {} ", a.to_lowercase())))
        .collect();
    phrases.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
    phrases
});

/// Single-word constellation names and genitives, lowercased, to lowercase abbreviation.
static WORDS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    CONSTELLATIONS
        .iter()
        .flat_map(|c| [c.name, c.genitive].map(|p| (p.to_lowercase(), c.abbr.to_lowercase())))
        .filter(|(p, _)| !p.contains(' '))
        .collect()
});

/// Strips common Latin accents (e.g. e-acute -> e) so names like Belenos match plain
/// typing and make clean URL slugs. Expects lowercase input.
pub fn fold_accent(c: char) -> char {
    match c {
        '\u{e0}'..='\u{e5}' | '\u{101}' => 'a',
        '\u{113}' => 'e',
        '\u{12b}' => 'i',
        '\u{14d}' => 'o',
        '\u{16b}' => 'u',
        '\u{e7}' => 'c',
        '\u{e8}'..='\u{eb}' => 'e',
        '\u{ec}'..='\u{ef}' => 'i',
        '\u{f1}' => 'n',
        '\u{f2}'..='\u{f6}' | '\u{f8}' => 'o',
        '\u{f9}'..='\u{fc}' => 'u',
        '\u{fd}' | '\u{ff}' => 'y',
        _ => c,
    }
}

/// Reduces a name or designation to a canonical search key, so that "Alpha Lyrae",
/// "alp Lyr", "α Lyr" and "ALPHA-LYR" all become "alp lyr", and "Alpha-1 Centauri"
/// becomes "alp1 cen".
pub fn normalize(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    for ch in text.chars() {
        if let Some((name, _, _)) = GREEK.iter().find(|g| g.2 == ch) {
            s.push(' ');
            s.push_str(name);
            s.push(' ');
        } else if ch.is_alphanumeric() {
            s.extend(ch.to_lowercase().map(fold_accent));
        } else {
            s.push(' ');
        }
    }
    let mut s = format!(" {} ", s.split_whitespace().collect::<Vec<_>>().join(" "));
    for (phrase, abbr) in PHRASES.iter() {
        if s.contains(phrase.as_str()) {
            s = s.replace(phrase.as_str(), abbr);
        }
    }

    let mut out: Vec<String> = Vec::new();
    for token in s.split_whitespace() {
        // A lone component digit after a Greek letter joins it: "alpha 1" -> "alp1".
        if token.len() == 1
            && token.as_bytes()[0].is_ascii_digit()
            && out.last().is_some_and(|t| greek_abbr(t).is_some())
        {
            out.last_mut().unwrap().push_str(token);
            continue;
        }
        let split = token.find(|c: char| c.is_ascii_digit()).unwrap_or(token.len());
        let (letters, digits) = token.split_at(split);
        if let Some(abbr) = greek_abbr(letters).filter(|_| digits.len() <= 1) {
            out.push(format!("{abbr}{digits}"));
            continue;
        }
        if let Some(abbr) = WORDS.get(token) {
            out.push(abbr.clone());
            continue;
        }
        out.push(if token == "gj" { "gl".into() } else { token.into() });
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_complete_and_unique() {
        let mut abbrs: Vec<_> = CONSTELLATIONS.iter().map(|c| c.abbr).collect();
        abbrs.sort();
        abbrs.dedup();
        assert_eq!(abbrs.len(), 88);
    }

    #[test]
    fn bayer_spellings_agree() {
        for q in ["Alpha Lyrae", "alp Lyr", "α Lyr", "ALPHA-LYR", "alpha lyra"] {
            assert_eq!(normalize(q), "alp lyr", "{q}");
        }
    }

    #[test]
    fn components_and_multiword() {
        assert_eq!(normalize("Alpha-1 Centauri"), "alp1 cen");
        assert_eq!(normalize("alpha 1 cen"), "alp1 cen");
        assert_eq!(normalize("Alp1 Cen"), "alp1 cen");
        assert_eq!(normalize("Alpha Canis Majoris"), "alp cma");
        assert_eq!(normalize("beta Trianguli Australis"), "bet tra");
        assert_eq!(normalize("Delta Delphini"), "del del");
    }

    #[test]
    fn catalog_prefixes() {
        assert_eq!(normalize("61 Cygni"), "61 cyg");
        assert_eq!(normalize("GJ 559A"), "gl 559a");
        assert_eq!(normalize("HIP  91262"), "hip 91262");
    }

    #[test]
    fn greek_display() {
        assert_eq!(greek_name("Alp").as_deref(), Some("Alpha"));
        assert_eq!(greek_name("Kap-2").as_deref(), Some("Kappa-2"));
        assert_eq!(greek_name("Xyz"), None);
    }
}

//! ADR 0042: the remake's own texts (its Options rows, mode labels, Settings, Quick game, History, Event log,
//! lobby, statistics, auto research, replays and other labels the original never had) in Hungarian and English.
//! Our own wording, keyed by the English text the code writes; other languages show the English. The game's own
//! texts keep coming from the install.

use ig2_formats::text::Lang;

/// (English, Hungarian). A new remake label joins with one row here and goes through [`get`] (or
/// `UiTexts::ours`) where it is drawn; the test below fails on a row without both languages.
const TABLE: &[(&str, &str)] = &[
    // Options: the remake's rows (display.rs) and their values (at most 10 letters, the value field).
    ("Restart to change the language", "A nyelv újraindítás után vált"),
    // The Remake page (ADR 0048): the tab's tooltip, the groups' headings, the sub-page row.
    ("REMAKE", "FELÚJÍTÁS"),
    ("The remake's own settings", "A felújítás saját beállításai"),
    ("Screen", "Képernyő"),
    ("Interface", "Kezelőfelület"),
    ("Game", "Játék"),
    ("Accessibility", "Akadálymentesség"),
    ("Page", "Oldal"),
    ("Display", "Megjelenítés"),
    ("Resolution", "Felbontás"),
    ("Vertical sync", "Függőleges szinkron"),
    ("Letterbox 4:3", "4:3 sávokkal"),
    ("Live background", "Élő háttér"),
    ("Names in game language", "Nevek a játék nyelvén"),
    ("Clean name entry", "Tiszta névbevitel"),
    ("Models inside boxes", "Modellek a keretben"),
    ("Slider under label", "Csúszka a felirat alatt"),
    ("Hungarian articles", "Magyar névelők"),
    ("Centred Exit label", "Középre igazított Kilépés"),
    ("WASD camera", "WASD kamera"),
    ("Autosave every", "Automentés"),
    ("Settings in menu", "Beállítások a menüben"),
    ("Language", "Nyelv"),
    ("Fleet groups", "Flottacsoportok"),
    ("Game statistics", "Játékstatisztika"),
    ("Record replays", "Visszajátszás rögzítése"),
    ("Event log", "Eseménynapló"),
    ("Subtitles", "Feliratok"),
    ("Colour-blind colours", "Színtévesztő színek"),
    ("Sharp UI", "Éles felület"),
    ("Sharp text", "Éles szöveg"),
    ("Starmap mouse camera", "Egérkamera a térképen"),
    ("High-score entry", "Rekordlista-bejegyzés"),
    ("Computer empires", "Gépi birodalmak"),
    ("Title music with intro", "Főcímzene az intróval"),
    ("Window", "Ablak"),
    ("Borderless", "Keretlen"),
    ("Fullscreen", "Teljes kép"),
    ("Default", "Alapérték"),
    ("Off", "Ki"),
    ("Starmap", "Térkép"),
    ("Both", "Mindkettő"),
    ("Original", "Eredeti"),
    ("10 days", "10 nap"),
    ("30 days", "30 nap"),
    ("90 days", "90 nap"),
    ("Classic", "Klasszikus"),
    ("Enhanced", "Fejlett"),
    // The research screen's auto research switches (label and state within 22 letters) and tooltips.
    ("Auto ships", "Auto hajók"),
    ("Auto buildings", "Auto épületek"),
    ("Auto tanks", "Auto tankok"),
    ("ON", "BE"),
    ("OFF", "KI"),
    ("Idle research picks ships, weapons, radars, shields, bombs", "Üres kutatáskor: hajók, fegyverek, radarok, pajzsok, bombák"),
    ("Idle research picks buildings and power plants", "Üres kutatáskor: épületek és erőművek"),
    ("Idle research picks tanks, turrets and mines", "Üres kutatáskor: tankok, lövegtornyok és aknák"),
    // The main menu.
    ("Settings", "Beállítások"),
    ("Quick game", "Gyors játék"),
    ("History", "Előzmények"),
    // Game statistics and the match history (column heads at most 7 letters).
    ("Victory", "Győzelem"),
    ("Defeat", "Vereség"),
    ("Game over", "Játék vége"),
    ("Campaign", "Hadjárat"),
    ("Single player", "Egyjátékos"),
    ("Multiplayer", "Többjátékos"),
    ("{}  -  {} game days (to {})  -  ended {}", "{}  -  {} játéknap ({}-ig)  -  vége: {}"),
    ("Empire", "Birodalom"),
    ("Planets", "Bolygók"),
    ("Pop.", "Népes."),
    ("Credits", "Kredit"),
    ("Ships", "Hajók"),
    ("Built", "Épült"),
    ("Lost", "Veszt."),
    ("Techs", "Tech"),
    ("Wins", "Győz."),
    ("Score", "Pont"),
    ("Population", "Népesség"),
    (" (you)", " (te)"),
    ("day {}", "{}. nap"),
    ("Match history", "Játéktörténet"),
    ("No finished games yet.", "Még nincs befejezett játék."),
    ("Date", "Dátum"),
    ("Mode", "Mód"),
    ("Result", "Eredmény"),
    ("Days", "Napok"),
    ("Empires", "Birodalmak"),
    ("Back", "Vissza"),
    ("Main menu", "Főmenü"),
    ("Continue", "Tovább"),
    ("Newer", "Újabbak"),
    ("Older", "Régebbiek"),
    // The messages screen's event log (the label at most 9 letters).
    ("EVENT LOG", "ESEMÉNYEK"),
    ("All", "Mind"),
    ("Battles", "Csaták"),
    ("Research", "Kutatás"),
    ("Diplomacy", "Diplomácia"),
    ("Colonies", "Kolóniák"),
    ("Story", "Történet"),
    ("Search:", "Keresés:"),
    ("No messages.", "Nincs üzenet."),
    // The multiplayer lobby and room.
    ("Host game", "Létrehozás"),
    ("Password (optional)", "Jelszó (nem kötelező)"),
    ("No games found on the local network", "Nem található játék a helyi hálózaton"),
    ("No internet matchmaking: join by address or join code", "Nincs internetes párosítás: csatlakozz címmel vagy kóddal"),
    ("Connecting...", "Kapcsolódás..."),
    ("computer", "számítógép"),
    ("closed", "zárva"),
    ("Galaxy shape:", "Galaxis alakja:"),
    ("Battles:", "Csaták:"),
    ("real time", "valós idejű"),
    ("space real time", "űrben valós idejű"),
    ("ground real time", "földön valós idejű"),
    ("automatic", "automatikus"),
    ("Speed control:", "Sebesség:"),
    ("everyone", "mindenki"),
    ("host", "a gazda"),
    ("Space battle formations:", "Űrcsata-alakzatok:"),
    ("off (classic)", "ki (klasszikus)"),
    ("every player picks", "mindenki maga választ"),
    ("Defeated players:", "Legyőzött játékosok:"),
    ("leave (classic)", "kilépnek (klasszikus)"),
    ("stay and watch", "maradnak és néznek"),
    ("Computer empires:", "Gépi birodalmak:"),
    ("classic", "klasszikus"),
    ("enhanced", "fejlett"),
    ("Two empires are needed to start", "Az indításhoz két birodalom kell"),
    ("Not everyone is ready: press again to start anyway", "Nem áll mindenki készen: nyomd meg újra az indításhoz"),
    ("Start anyway", "Indítás így is"),
    ("Not ready", "Nem kész"),
    ("Host", "Gazda"),
    ("Saved game", "Mentett játék"),
    ("LAN games", "LAN-játékok"),
    ("No multiplayer saves", "Nincs többjátékos mentés"),
    ("Pick one of the saved empires", "Válassz egyet a mentett birodalmak közül"),
    ("Port mapping (UPnP / NAT-PMP): off", "Porttovábbítás (UPnP / NAT-PMP): ki"),
    ("Port mapping: asking the router...", "Porttovábbítás: kérés a routertől..."),
    ("Port mapping: on, reachable at {}", "Porttovábbítás: be, elérhető itt: {}"),
    ("Port mapping failed: {}", "Porttovábbítás sikertelen: {}"),
    ("Private game: no", "Zárt játék: nem"),
    ("Private game: yes", "Zárt játék: igen"),
    ("Join code: none (no network address)", "Csatlakozási kód: nincs (nincs hálózati cím)"),
    ("Join code: {}  (click: copy)", "Csatlakozási kód: {}  (kattintás: másolás)"),
    ("The join code is on the clipboard", "A csatlakozási kód a vágólapon van"),
    ("Relay server (optional): ", "Közvetítő (nem kötelező):"),
    ("Address or join code:", "Cím vagy csatlakozási kód:"),
    ("Relay server: {}", "Közvetítő szerver: {}"),
    ("Relay server: opening the room...", "Közvetítő szerver: szoba nyitása..."),
    ("Relay server: room open, seen at {}", "Közvetítő szerver: a szoba nyitva, itt látható: {}"),
    ("Relay server: full, try later", "Közvetítő szerver: megtelt, próbáld később"),
    ("Relay server: refused the room", "Közvetítő szerver: elutasította a szobát"),
    ("Relay server: no answer", "Közvetítő szerver: nem válaszol"),
    ("Can't host: {}", "Nem indítható: {}"),
    ("Starting story set to {}", "Kezdő történet: {}"),
    ("Another game version", "Más játékverzió"),
    ("Another game install (rules differ)", "Más telepítés (eltérő szabályok)"),
    ("Other mods enabled (mods differ)", "Más modok bekapcsolva (eltérő modok)"),
    ("Wrong join code", "Hibás csatlakozási kód"),
    ("The game is full", "A játék megtelt"),
    ("The game has started", "A játék már elindult"),
    ("Wrong password", "Hibás jelszó"),
    ("Removed by the host", "A gazda eltávolított"),
    ("Connection refused", "A kapcsolat elutasítva"),
    ("Timed out", "Időtúllépés"),
    ("The game needs a password", "A játékhoz jelszó kell"),
    ("That player is connected", "Ez a játékos már csatlakozott"),
    // Multiplayer notices on the message line.
    ("{} paused the game.", "{} szüneteltette a játékot."),
    ("{} resumed the game.", "{} folytatta a játékot."),
    ("{} lost the connection: waiting {} s for a rejoin.", "{} kapcsolata megszakadt: {} mp várakozás a visszatérésre."),
    ("{} has left the game.", "{} kilépett a játékból."),
    ("{} is back.", "{} visszatért."),
    ("{} is watching the game.", "{} nézi a játékot."),
    ("{}'s game was out of step and has been resynchronised.", "{} játéka elcsúszott, újraszinkronizálva."),
    ("This game was out of step with the host and has been resynchronised.", "Ez a játék elcsúszott a gazdáétól, újraszinkronizálva."),
    ("The host has left the game. {} hosts it now.", "A gazda kilépett. Mostantól {} a gazda."),
    ("The host has kicked you from the game.", "A gazda kizárt a játékból."),
    ("The host has left the game.", "A gazda kilépett a játékból."),
    ("Your empire has fallen. You can watch the game until you leave it.", "A birodalmad elbukott. Kilépésig nézheted a játékot."),
    ("Waiting for the other players", "Várakozás a többi játékosra"),
    // Saves.
    ("Cannot save: {}", "Nem sikerült menteni: {}"),
    ("Quicksave {}", "Gyorsmentés {}"),
    ("Quick saved.", "Gyorsmentés kész."),
    ("Quick loaded.", "Gyorsbetöltés kész."),
    ("Autosave {}", "Automentés {}"),
];

/// The game's language for the lines made where no texts are at hand (the message line's own notices); English
/// until the client opens the install.
static GAME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn set_game(lang: Lang) {
    GAME.store(Lang::ALL.iter().position(|&l| l == lang).unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
}

pub fn game() -> Lang {
    Lang::ALL[GAME.load(std::sync::atomic::Ordering::Relaxed)]
}

/// `en` in `lang` as the font's bytes (the language's code page): the table's Hungarian for a Hungarian game, the
/// English otherwise, and `en` itself when the table lacks it.
pub fn get(lang: Lang, en: &str) -> Vec<u8> {
    ig2_formats::text::encode(text(lang, en), lang)
}

/// [`get`] as text.
pub fn text(lang: Lang, en: &str) -> &str {
    match lang {
        Lang::Hu => TABLE.iter().find(|(e, _)| *e == en).map_or(en, |(_, hu)| hu),
        _ => en,
    }
}

/// [`get`] of a template, each `{}` replaced by the next of `args` (already the font's bytes: names, numbers).
pub fn fill(lang: Lang, en: &str, args: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut args = args.iter();
    for (i, part) in text(lang, en).split("{}").enumerate() {
        if i > 0 {
            out.extend_from_slice(args.next().copied().unwrap_or_default());
        }
        out.extend(ig2_formats::text::encode(part, lang));
    }
    out
}

/// [`get`] for a label already in bytes (rows and lines built before the language is known store their English);
/// a line made from a template ending in `{}` ("Can't host: {}") keeps its tail (an address, a system error).
pub fn bytes(lang: Lang, en: &[u8]) -> Vec<u8> {
    let Ok(s) = std::str::from_utf8(en) else { return en.to_vec() };
    if lang != Lang::Hu {
        return en.to_vec();
    }
    if let Some((_, hu)) = TABLE.iter().find(|(e, _)| *e == s) {
        return ig2_formats::text::encode(hu, lang);
    }
    fn tail(&(e, hu): &(&'static str, &'static str)) -> Option<(&'static str, &'static str)> {
        Some((e.strip_suffix("{}")?, hu.strip_suffix("{}")?)).filter(|(p, _)| !p.is_empty() && !p.contains("{}"))
    }
    match TABLE.iter().filter_map(tail).filter(|(p, _)| s.starts_with(p)).max_by_key(|(p, _)| p.len()) {
        Some((p, hu)) => [ig2_formats::text::encode(hu, lang), en[p.len()..].to_vec()].concat(),
        None => en.to_vec(),
    }
}

/// The table has `en`.
#[cfg(test)]
pub fn has(en: &str) -> bool {
    TABLE.iter().any(|(e, _)| *e == en)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first string literal after each of `marks` in `src`.
    fn literals_after<'a>(src: &'a str, marks: &[&str]) -> Vec<&'a str> {
        let mut out = Vec::new();
        for m in marks {
            for (at, _) in src.match_indices(m) {
                let rest = &src[at + m.len()..];
                let Some(open) = rest.find('"').filter(|&o| !rest[..o].contains([';', '\n'])) else { continue };
                let body = &rest[open + 1..];
                out.push(&body[..body.find('"').unwrap()]);
            }
        }
        out
    }

    /// Every remake label the client routes through the table is in it.
    #[test]
    fn the_routed_labels_are_in_the_table() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = Vec::new();
        for e in std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "rs")) {
            let src = std::fs::read_to_string(e.path()).unwrap();
            let src = src.split("#[cfg(test)]").next().unwrap();
            let mut found = literals_after(src, &["ours(", "tr::get(", "tr::fill(", "say("]);
            if e.file_name() == "mp_lobby.rs" {
                let words = &src[src.find("mod words {").unwrap()..];
                let refusals = &src[src.find("fn refusal_text").unwrap()..];
                for part in [&words[..words.find("\n}").unwrap()], &refusals[..refusals.find("\n}").unwrap()]] {
                    found.extend(part.split("b\"").skip(1).map(|s| &s[..s.find('"').unwrap()]).filter(|&s| s != "-"));
                }
            }
            missing.extend(found.into_iter().filter(|s| !s.is_empty() && !has(s)).map(|s| format!("{}: {s:?}", e.file_name().to_string_lossy())));
        }
        let langs = ["English", "Magyar", "Deutsch", "Francais", "Espanol", "Italiano"];
        for r in crate::display::rows(&Default::default(), &[(800, 600)], (800, 600)) {
            let name = String::from_utf8(r.name).unwrap();
            let labels = r.labels.into_iter().map(|l| String::from_utf8(l).unwrap()).filter(|l| !l.contains('x') && !langs.contains(&l.as_str()));
            missing.extend(std::iter::once(name).chain(labels).filter(|s| !has(s)));
        }
        missing.extend(crate::event_log::FILTERS.iter().map(|f| f.0.to_string()).filter(|s| !has(s)));
        missing.extend(crate::options::Group::ALL.iter().map(|g| g.title().to_string()).filter(|s| !has(s)));
        assert!(missing.is_empty(), "not in the table: {missing:#?}");
    }

    /// Every Hungarian letter of the table has a glyph in the Hungarian font (`charset.order_hu`).
    #[test]
    fn the_hungarian_font_has_every_letter() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(Lang::Hu) else { return };
        let rules = ig2_rules::Rules::from_install(&install).unwrap();
        for (_, hu) in TABLE {
            for b in get(Lang::Hu, &hu.replace("{}", "")) {
                assert!(rules.charset.order_hu.contains(&b), "{hu:?}: byte {b:#x} has no glyph");
            }
        }
    }

    #[test]
    fn every_key_has_hungarian_and_english() {
        let mut seen = std::collections::HashSet::new();
        for (en, hu) in TABLE {
            assert!(!en.trim().is_empty() && !hu.trim().is_empty(), "{en:?} / {hu:?}");
            assert!(seen.insert(*en), "{en:?} twice");
            // The font's code page holds every Hungarian letter used.
            let (_, _, bad) = Lang::Hu.encoding().encode(hu);
            assert!(!bad, "{hu:?} not in the Hungarian code page");
        }
    }

    #[test]
    fn other_languages_fall_back_to_english() {
        assert_eq!(text(Lang::De, "Event log"), "Event log");
        assert_eq!(text(Lang::Hu, "not a remake label"), "not a remake label");
        assert_eq!(fill(Lang::En, "{} is back.", &[b"Kim"]), b"Kim is back.");
        assert_eq!(bytes(Lang::De, b"Event log"), b"Event log");
        assert_eq!(bytes(Lang::Hu, b"Can't host: denied"), [get(Lang::Hu, "Can't host: {}").strip_suffix(b"{}").unwrap(), b"denied"].concat());
    }
}

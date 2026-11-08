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
    ("Every screen in the modern panels and pages, or in the original's look; switches at once.", "Minden képernyő modern paneleken és oldalakon, vagy az eredeti kinézetben; azonnal vált."),
    ("Game", "Játék"),
    ("Accessibility", "Akadálymentesség"),
    ("Graphics", "Grafika"),
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
    ("Autosave every", "Automatikus mentés gyakorisága"),
    ("Settings in menu", "Beállítások a menüben"),
    ("Language", "Nyelv"),
    ("Fleet groups", "Flottacsoportok"),
    ("Game statistics", "Játékstatisztika"),
    ("Record replays", "Visszajátszás rögzítése"),
    ("Event log", "Eseménynapló"),
    ("Subtitles", "Feliratok"),
    ("Colour-blind colours", "Színtévesztőbarát színek"),
    ("Sharp UI", "Éles felület"),
    ("Sharp text", "Éles szöveg"),
    ("Texture upscaling", "Textúra-felskálázás"),
    ("Anti-aliasing", "Élsimítás"),
    ("Per-pixel lighting", "Pixelenkénti megvilágítás"),
    ("Soft shadows", "Lágy árnyékok"),
    ("Bloom", "Ragyogás"),
    ("UI theme", "Felület témája"),
    ("Modern", "Modern"),
    ("The look of the remake's own screens; Classic keeps the game's colours.", "A felújítás saját képernyőinek kinézete; a Klasszikus a játék színeit tartja meg."),
    ("Battle effects", "Csataeffektusok"),
    ("Smooth glows", "Lágy fények"),
    ("UI scale", "Felület mérete"),
    ("The size of the remake's own pages and panels; they grow with the window.", "A remake saját oldalainak és paneljeinek mérete; az ablakkal együtt nőnek."),
    // The modern starmap HUD (ADR 0065).
    ("Speed", "Sebesség"),
    // The modern fleet panel (ADR 0077).
    ("Equipment", "Felszerelés"),
    ("Ship info", "Hajó info"),
    ("Quick research", "Gyors kutatás"),
    ("Build cursor", "Építési kurzor"),
    ("No research", "Nincs kutatás"),
    ("Redirect", "Átirányítás"),
    ("Details", "Infó"),
    ("Build", "Építés"),
    ("Attacker", "Támadó"),
    ("Defender", "Védő"),
    ("Split", "Felosztás"),
    ("Richness", "Gazdagság"),
    ("Habitability", "Lakhatóság"),
    ("Menu navigation", "Menünavigáció"),
    ("A grouped main menu with Continue; Back, Esc or right click on every menu screen.", "Csoportos főmenü Tovább gombbal; Vissza, Esc vagy jobb klikk minden menüben."),
    ("Starmap mouse camera", "Egérkamera a térképen"),
    ("High-score entry", "Rekordlista-bejegyzés"),
    ("Computer empires", "Gépi birodalmak"),
    ("Campaign story", "Kampánytörténet"),
    ("Title music with intro", "Főcímzene az intróval"),
    ("Window", "Ablak"),
    ("Borderless", "Keret nélküli"),
    ("Fullscreen", "Teljes képernyő"),
    ("Default", "Alapérték"),
    ("Off", "Ki"),
    ("Starmap", "Térkép"),
    ("Both", "Mindkettő"),
    ("Original", "Eredeti"),
    ("10 days", "10 nap"),
    ("30 days", "30 nap"),
    ("90 days", "90 nap"),
    ("Classic", "Klasszikus"),
    ("Enhanced", "Bővített"),
    // The main menu's Options screen (ADR 0053): title, tabs (at most 13 letters), buttons, the settings' descriptions.
    ("OPTIONS", "BEÁLLÍTÁSOK"),
    ("DISPLAY", "MEGJELENÍTÉS"),
    ("SOUND", "HANG"),
    ("INTERFACE", "FELÜLET"),
    ("GRAPHICS", "GRAFIKA"),
    ("Effects", "Effektek"),
    ("Picture", "Kép"),
    ("Modern interface", "Modern felület"),
    ("Classic interface", "Klasszikus felület"),
    ("Borderless uses the desktop's size.", "Keret nélkül az asztal méretét használja."),
    ("Used by the Modern interface.", "A modern felület használja."),
    ("Used by the Classic interface.", "A klasszikus felület használja."),
    ("Modern always uses the grouped menu.", "A modern mindig a csoportos menüt használja."),
    ("GAME", "JÁTÉK"),
    ("ACCESSIBILITY", "AKADÁLYMENTESSÉG"),
    ("MULTIPLAYER", "TÖBBJÁTÉKOS"),
    ("LANGUAGE", "NYELV"),
    ("Apply", "Alkalmaz"),
    ("Player name", "Játékosnév"),
    ("Leave; changes not applied are taken back.", "Kilépés; a nem alkalmazott változások elvesznek."),
    ("Save the changes and use them.", "A változások mentése és használata."),
    ("Saves the game into slot 24 at the start of January and July.", "Január és július elején a 24. helyre menti a játékot."),
    ("Ground battles are fought out by the computer, without the battle screen.", "A földi csatákat a gép vívja meg, csatakép nélkül."),
    ("Overwhelming forces win ground battles without them being fought.", "A nagy túlerő harc nélkül nyeri meg a földi csatát."),
    ("Space battles are fought out by the computer, without the battle screen.", "Az űrcsatákat a gép vívja meg, csatakép nélkül."),
    ("Overwhelming fleets win space battles without them being fought.", "A nagy túlerejű flotta harc nélkül nyeri meg az űrcsatát."),
    ("Plays the story's video scenes.", "Lejátssza a történet videójeleneteit."),
    ("A switch of the original without a known effect.", "Az eredeti játék kapcsolója, ismert hatás nélkül."),
    ("Screens open and close with their animation.", "A képernyők animációval nyílnak és zárulnak."),
    ("Loads the smaller textures.", "A kisebb textúrákat tölti be."),
    ("Shows the buildings' names on the colony screen.", "Kiírja az épületek nevét a kolónia képernyőjén."),
    ("Draws shadows under the colony's buildings and units.", "Árnyékot rajzol a kolónia épületei és egységei alá."),
    ("Draws the roads between the colony's buildings.", "Megrajzolja az utakat a kolónia épületei között."),
    ("Smoke, dust and sparks on the colony screen.", "Füst, por és szikrák a kolónia képernyőjén."),
    ("Rain, snow and other weather on the colony screen.", "Eső, hó és más időjárás a kolónia képernyőjén."),
    ("The detail of the colony's building models.", "A kolónia épületmodelljeinek részletessége."),
    ("Sharper line drawings on the research screen.", "Élesebb vonalrajzok a kutatás képernyőjén."),
    ("How many nebula clouds the starmap draws.", "Hány ködfelhőt rajzol a csillagtérkép."),
    ("The music's loudness.", "A zene hangereje."),
    ("The picture's brightness, where the display allows it.", "A kép fényereje, ahol a kijelző engedi."),
    ("Your name in multiplayer games, at most 9 letters.", "A neved a többjátékos játékokban, legfeljebb 9 betű."),
    ("A window, a borderless window over the screen, or fullscreen.", "Ablak, keret nélküli ablak a teljes képernyőn, vagy teljes képernyős mód."),
    ("The size of the window or of the fullscreen mode.", "Az ablak vagy a teljes képernyős mód mérete."),
    ("Waits for the display's refresh, so the picture doesn't tear.", "Megvárja a kijelző frissítését, így a kép nem szakad."),
    ("Keeps the original 4:3 picture with bars at the sides.", "Megtartja az eredeti 4:3-as képet, oldalt sávokkal."),
    ("The game moves on visibly behind the screens over it.", "A játék láthatóan fut tovább a fölötte nyíló képernyők mögött."),
    ("Empire, planet and other names in the game's language.", "A birodalmak, bolygók és más nevek a játék nyelvén."),
    ("A save's old name isn't shown under the one being typed.", "Mentéskor a régi név nem látszik a beírt alatt."),
    ("The info boxes' turning models stay inside their boxes.", "Az infódobozok forgó modelljei a keretben maradnak."),
    ("The space battle's attack slider sits under its label.", "Az űrcsata támadási csúszkája a felirata alatt van."),
    ("The right a/az article before names in Hungarian texts.", "Helyes a/az névelő a nevek előtt a magyar szövegekben."),
    ("The starmap panels' Exit label is centred on its button.", "A térkép paneljein a Kilépés felirat középen áll."),
    ("W, A, S and D move the camera on the starmap, or on colonies too.", "A W, A, S, D mozgatja a kamerát a térképen, vagy a kolóniákon is."),
    ("How often the game saves itself (Original: twice a year).", "Milyen gyakran ment a játék magától (Eredeti: évente kétszer)."),
    ("The main menu's Settings label and the O key.", "A főmenü Beállítások felirata és az O billentyű."),
    ("The texts' language, from the next start.", "A szövegek nyelve, a következő indítástól."),
    ("Ctrl+1..9 stores the selected fleet, Alt+1..9 selects it.", "Ctrl+1..9 eltárolja a kijelölt flottát, Alt+1..9 kijelöli."),
    ("A statistics screen after each game, and the match history.", "Statisztika minden játék végén, és a korábbi játékok listája."),
    ("Each game is recorded and can be watched from the main menu.", "Minden játék rögzül, és a főmenüből visszanézhető."),
    ("The messages screen keeps a log of the game's events.", "Az üzenetek képernyője naplót vezet a játék eseményeiről."),
    ("The spoken messages' text at the bottom of the screen.", "A beszélt üzenetek szövege a képernyő alján."),
    ("Empire colours that stay apart with colour blindness.", "Színtévesztéssel is megkülönböztethető birodalomszínek."),
    ("Pictures and panels stay sharp on big screens.", "A képek és panelek nagy képernyőn is élesek."),
    ("Letters with crisp, smooth edges on big screens.", "Éles, sima szélű betűk nagy képernyőn."),
    ("Smooth edges on the 3D scenes' buildings, ships and planets.", "Sima élek a 3D épületeken, hajókon, bolygókon."),
    ("The space battle's ships lit per pixel, so their hulls shade smoothly.", "Az űrcsata hajói pixelenként megvilágítva, simán árnyalva."),
    ("Soft edges on the colony's building shadows.", "Lágy szélű árnyékok a kolónia épületei alatt."),
    ("A soft glow around fire, blasts and the colony's weather.", "Lágy ragyogás a tűz, a robbanások és az időjárás körül."),
    ("The wheel zooms the starmap in and out.", "A görgő nagyítja és kicsinyíti a csillagtérképet."),
    ("A won scenario's time goes into its high-score table.", "A megnyert küldetés ideje bekerül a rekordlistába."),
    ("How the computer empires of the next game manage their economy and research.", "Hogyan intézik a gépi birodalmak gazdaságukat és kutatásukat a következő játékban."),
    ("The title video's music starts with the intro, not under the logos.", "A címvideó zenéje a bevezetővel indul, nem a logók alatt."),
    ("Enhanced fixes the original campaign story's slips in the next new game.", "Bővített: a következő új játékban javítja az eredeti kampánytörténet hibáit."),
    ("Smooths the game's pictures and model textures 2x or 4x as they load.", "Betöltéskor 2x vagy 4x-esére nagyítja és simítja a játék képeit és modelltextúráit."),
    ("Sparks, shock waves, smoke and glowing shots on top of the battles' own effects.", "Szikrák, lökéshullámok, füst és izzó lövések a csaták saját effektusai fölött."),
    ("Fleets", "Flották"),
    ("Name", "Név"),
    ("List order", "Eredeti sorrend"),
    ("Size", "Méret"),
    ("Alerts", "Riasztás"),
    ("SAVE & LOAD", "MENTÉS ÉS BETÖLTÉS"),
    ("Saves", "Mentések"),
    ("Selected save", "Kiválasztott mentés"),
    ("Empty", "Üres"),
    ("Empty slot", "Üres hely"),
    ("Load", "Betöltés"),
    ("Save", "Mentés"),
    ("Cancel", "Mégse"),
    ("Exit", "Kilépés"),
    ("Quit to main menu", "Kilépés a főmenübe"),
    ("Quit the tutorial", "Kilépés az oktatásból"),
    ("Exit game", "Kilépés a játékból"),
    ("Continue tutorial", "Az oktatás folytatása"),
    ("Resume", "Folytatás"),
    ("Everywhere: screens", "Mindenhol: képernyők"),
    ("Everywhere: time and saving", "Mindenhol: idő és mentés"),
    ("Enter accepts, Esc goes back.", "Enter: elfogad, Esc: vissza."),
    ("Quit to the main menu? Play since the last save is lost.", "Kilépés a főmenübe? Ami az utolsó mentés óta történt, elvész."),
    ("Exit the game? Play since the last save is lost.", "Kilépés a játékból? Ami az utolsó mentés óta történt, elvész."),
    ("Game date: {}", "Játékbeli dátum: {}"),
    ("Slot {}", "{}. hely"),
    // The production view's slots (ADR 0071).
    ("Free slot", "Szabad hely"),
    ("No design chosen", "Tervrajz nincs kiválasztva"),
    ("Blueprint", "Tervrajz"),
    ("Repeat", "Ismétlés"),
    ("Into fleet", "Flottába"),
    ("Autosave", "Automatikus mentés"),
    ("Previous autosave", "Előző automatikus mentés"),
    ("Saved just now", "Utolsó mentés: épp most"),
    ("Saved {} minutes ago", "Utolsó mentés: {} perce"),
    ("Saved {} hours ago", "Utolsó mentés: {} órája"),
    ("Saved {} days ago", "Utolsó mentés: {} napja"),
    ("Save name", "A mentés neve"),
    // The options boards' cards and the save and load page (ADR 0078).
    ("Detail", "Részletek"),
    ("Readability", "Olvashatóság"),
    ("Subtitles and colours", "Feliratok és színek"),
    ("Volume", "Hangerő"),
    ("Music", "Zene"),
    ("Battles and saving", "Csaták és mentés"),
    ("Enhancements", "Bővítések"),
    ("Handling", "Kezelés"),
    ("Status", "Állapot"),
    ("Join code", "Csatlakozási kód"),
    ("Cutscenes", "Átvezető videók"),
    ("View Rotation", "Nézet forgatása"),
    ("Game date", "Játékbeli dátum"),
    ("Slot", "Hely"),
    ("Saved", "Mentve"),
    ("Saving here overwrites the old save.", "Mentés erre a helyre: a régi mentés felülíródik."),
    ("just now", "az imént"),
    ("{} minutes ago", "{} perce"),
    ("{} hours ago", "{} órája"),
    ("{} days ago", "{} napja"),
    ("Enter saves, Esc cancels.", "Enter: mentés, Esc: mégse."),
    ("Multiplayer: no loading; only the host saves.", "Többjátékos: nincs betöltés, csak a gazda ment."),
    ("No saving during a battle.", "Csata közben nem lehet menteni."),
    ("Soft, round glows and stars on the starmap and the backgrounds at any resolution.", "Lágy, kerek fények és csillagok a csillagtérképen és a hátterekben, bármely felbontáson."),
    // The research screen's auto research switches (label and state within 22 letters) and tooltips.
    ("Auto ships", "Auto hajók"),
    ("Auto buildings", "Auto épületek"),
    ("Auto tanks", "Auto tankok"),
    ("ON", "BE"),
    // The modern research screen (ADR 0071).
    ("Auto research", "Automatikus kutatás"),
    ("Buildings", "Épületek"),
    ("Tanks", "Tankok"),
    ("Stationed tanks", "Állomásozó tankok"),
    ("Tanks and turrets", "Tankok és tornyok"),
    ("Done", "Kész"),
    ("Locked", "Zárt"),
    ("Funding", "Finanszírozás"),
    ("Double money", "Dupla pénz"),
    ("Labs", "Laborok"),
    // The modern design screen (ADR 0075).
    ("Blueprints", "Tervrajzok"),
    ("Parts", "Alkatrészek"),
    ("POD", "POD"),
    ("Choose", "Választás"),
    ("New", "Új"),
    ("Name in use", "A név már foglalt"),
    ("Delete", "Törlés"),
    ("OK", "Rendben"),
    ("Auto", "Automatikus"),
    ("Clear", "Ürítés"),
    ("Ships and weapons", "Hajók és fegyverek"),
    ("OFF", "KI"),
    ("Idle research picks ships, weapons, radars, shields, bombs", "Üres kutatáskor: hajók, fegyverek, radarok, pajzsok, bombák"),
    ("Idle research picks buildings and power plants", "Üres kutatáskor: épületek és erőművek"),
    ("Idle research picks tanks, turrets and mines", "Üres kutatáskor: tankok, lövegtornyok és aknák"),
    // The main menu.
    ("Settings", "Beállítások"),
    ("Quick game", "Gyors játék"),
    // The Custom game page (ADR 0085).
    ("Custom game", "Egyéni játék"),
    ("Galaxy", "Galaxis"),
    ("Seed", "Kezdőszám"),
    ("New seed", "Új kezdőszám"),
    // The classic skirmish panel's label and the multiplayer room's switch (ADR 0085 §4, §5).
    ("Advanced", "Haladó"),
    ("on", "be"),
    ("The same seed and settings make the same galaxy.", "Ugyanaz a kezdőszám és beállítás ugyanazt a galaxist adja."),
    ("Computer", "Gép"),
    ("Aggressiveness", "Agresszivitás"),
    ("Relations", "Kapcsolat"),
    ("Low", "Alacsony"),
    ("High", "Magas"),
    ("Hostile", "Ellenséges"),
    ("Neutral", "Semleges"),
    ("Friendly", "Baráti"),
    ("Remove", "Eltávolítás"),
    ("Add computer empire", "Gépi birodalom hozzáadása"),
    ("Start position", "Kezdőhely"),
    ("Random start", "Véletlen kezdőhely"),
    ("Click a star to start there.", "Kattints egy csillagra: ott kezdesz."),
    ("You start on the star you picked.", "A választott csillagon kezdesz."),
    ("Your start", "A kezdőhelyed"),
    ("History", "Előzmények"),
    // The enhanced menu navigation (ADR 0059).
    ("Play", "Játék"),
    ("New game", "Új játék"),
    ("Load game", "Betöltés"),
    ("Extras", "Extrák"),
    ("Replays", "Visszajátszások"),
    // The mod manager (ADR 0068).
    ("Mods", "Modok"),
    ("MODS", "MODOK"),
    ("No mods found. Put a mod's folder or .zip file into the mods folder, then open this page again.", "Nincs mod. Tegyél egy mod mappát vagy .zip fájlt a mods mappába, majd nyisd meg újra ezt az oldalt."),
    ("by", "készítette:"),
    ("zip", "zip"),
    ("folder", "mappa"),
    ("Made for openIG2", "Ehhez az openIG2-höz készült:"),
    ("of its files come from", "fájlja ebből jön:"),
    ("Load this mod at the next start.", "A következő indításkor betölti ezt a modot."),
    ("Lower priority", "Kisebb elsőbbség"),
    ("Higher priority: its files win", "Nagyobb elsőbbség: az ő fájljai győznek"),
    ("This run's mods come from --mod; the list applies without it.", "Most a --mod adja a modokat; a lista nélküle érvényes."),
    ("Apply saves the list; it loads at the next start.", "Az Alkalmaz menti a listát; a következő indításkor töltődik be."),
    ("Restart to use the changed mods.", "Indítsd újra a játékot a módosított modokhoz."),
    ("Save the list of enabled mods.", "Menti a bekapcsolt modok listáját."),
    ("Restart now", "Újraindítás most"),
    ("Open mods folder", "Mods mappa megnyitása"),
    ("The mods folder beside the game: drop a mod's .zip file here.", "A játék melletti mods mappa: ide tedd a mod .zip fájlját."),
    ("priority", "elsőbbség"),
    ("look only", "csak külső"),
    ("Changes only how the game looks: multiplayer rooms don't need it.", "Csak a játék kinézetét változtatja: a többjátékos szobáknak nem kell."),
    ("Needs:", "Kell hozzá:"),
    ("Can't load: it", "Nem tölthető be:"),
    ("A mod can't load: switch it off or add what it needs.", "Egy mod nem tölthető be: kapcsold ki, vagy add hozzá, ami kell neki."),
    ("How to make a mod", "Hogyan készíts modot"),
    ("The modding tutorial on the openIG2 website, in your browser.", "A modkészítési útmutató az openIG2 oldalán, a böngésződben."),
    // The Mods page's Browse tab (ADR 0086 §1.2).
    ("Browse", "Böngészés"),
    ("The mods in the mods folders (Ctrl+Tab).", "A mods mappákban lévő modok (Ctrl+Tab)."),
    ("The openIG2 mod repository: fetched from the internet when opened (Ctrl+Tab).", "Az openIG2 modtára: megnyitáskor az internetről töltődik le (Ctrl+Tab)."),
    ("Browse fetches the mod list from github.com/openIG2-org/mods (only when this tab opens).", "A Böngészés a github.com/openIG2-org/mods oldalról tölti le a modlistát (csak amikor ez a fül megnyílik)."),
    ("Fetching the mod list from github.com/openIG2-org/mods...", "A modlista letöltése a github.com/openIG2-org/mods oldalról..."),
    ("Fetching the mod list...", "A modlista letöltése..."),
    ("Signed mod list from github.com/openIG2-org/mods:", "Aláírt modlista a github.com/openIG2-org/mods oldalról:"),
    ("Offline, the saved mod list:", "Offline, a mentett modlista:"),
    ("The mod list didn't load:", "A modlista nem töltődött be:"),
    ("No mod matches the search.", "Egy mod sem felel meg a keresésnek."),
    ("No mods in the list yet for openIG2", "Még nincs mod a listán ehhez az openIG2-höz:"),
    ("No mods folder here.", "Itt nincs mods mappa."),
    ("Search mods", "Modok keresése"),
    ("Fetch the mod list again (F5).", "A modlista újbóli letöltése (F5)."),
    ("Install", "Telepítés"),
    ("Installing", "Telepítés:"),
    ("Installed", "Telepítve"),
    ("Installed by hand (a folder)", "Kézzel telepítve (mappa)"),
    ("installed:", "telepítve:"),
    ("installed with it", "vele együtt települ"),
    ("Update", "Frissítés"),
    ("Updating", "Frissítés:"),
    ("Removed", "Eltávolítva:"),
    ("Can't load with:", "Nem tölthető be ezekkel:"),
    ("Download, check and put it into the mods folder, with the mods it needs.", "Letölti, ellenőrzi és a mods mappába teszi a szükséges modokkal együtt."),
    ("Download this release; the installed zip stays until the new one is checked.", "Letölti ezt a kiadást; a telepített zip megmarad, amíg az újat ellenőrzi."),
    ("Delete its zip from the mods folder.", "Törli a zip fájlját a mods mappából."),
    ("Apply or take back the changed list on Installed first.", "Előbb alkalmazd vagy vond vissza a Telepítve fülön módosított listát."),
    ("Mods in this game:", "A játék modjai:"),
    ("The mods failed to load; the game started without them:", "A modok betöltése nem sikerült; a játék nélkülük indult:"),
    ("Quit", "Kilépés"),
    ("Tutorial", "Oktatás"),
    ("Game credits", "Készítők"),
    ("Scenarios", "Forgatókönyvek"),
    ("Local network", "Helyi hálózat"),
    ("Internet", "Internet"),
    ("Race", "Faj"),
    ("Started (UTC)", "Kezdés (UTC)"),
    ("No recorded games yet.", "Még nincs rögzített játék."),
    ("Selected replay", "Kiválasztott visszajátszás"),
    ("No replay selected", "Nincs kiválasztott visszajátszás"),
    ("Watch", "Megtekintés"),
    ("A double click or Watch plays the replay; Esc goes back.", "Dupla kattintás vagy a Megtekintés lejátssza; Esc: vissza."),
    ("Part 1", "1. rész"),
    ("Part 2", "2. rész"),
    ("Part 3", "3. rész"),
    ("Part 4", "4. rész"),
    ("Part 5", "5. rész"),
    ("The starmap: the camera, planets and fleets", "A csillagtérkép: a kamera, a bolygók és a flották"),
    ("Colony management: buildings on a new colony", "Kolóniaszervezés: épületek egy új kolónián"),
    ("Production, research and ship design", "Gyártás, kutatás és hajótervezés"),
    ("Space and ground battles", "Űr- és földi csaták"),
    ("Trade, diplomacy and spying", "Kereskedelem, diplomácia és kémkedés"),
    ("Difficulty", "Nehézség"),
    ("Easy", "Könnyű"),
    ("Normal", "Normál"),
    ("Hard", "Nehéz"),
    // The modern new game page (ADR 0078).
    ("Pick an empire, then the difficulty. The campaign plays the chosen race's story.", "Válassz birodalmat, aztán nehézséget. A kampány a választott faj történetét játssza végig."),
    ("Start", "Indítás"),
    // The modern help page (ADR 0078).
    ("Search", "Keresés"),
    ("No topic found.", "Nincs ilyen téma."),
    // The modern end-of-game page (ADR 0078).
    ("{} game days", "{} játéknap"),
    ("ended {}", "vége: {}"),
    // Game statistics and the match history (column heads at most 7 letters).
    ("Victory", "Győzelem"),
    ("Defeat", "Vereség"),
    ("Game over", "Játék vége"),
    ("Campaign", "Kampány"),
    ("Single player", "Egyjátékos"),
    ("Multiplayer", "Többjátékos"),
    ("{}  -  {} game days (to {})  -  ended {}", "{}  -  {} játéknap ({}-ig)  -  vége: {}"),
    ("Empire", "Birodalom"),
    ("Planets", "Bolygók"),
    ("Pop.", "Népes."),
    ("Credits", "Kredit"),
    ("Ships", "Hajók"),
    ("Orders", "Parancsok"),
    ("Morale", "Morál"),
    ("new", "új"),
    ("Sender", "Feladó"),
    ("Full text", "Teljes szöveg"),
    ("Contacts", "Kapcsolatok"),
    ("Skill points", "Képességpontok"),
    ("No offers", "Nincs ajánlat"),
    ("None", "Nincs"),
    ("Total", "Összesen"),
    ("Deliveries under way", "Úton lévő szállítmányok"),
    ("Yellow: unread", "Sárga: olvasatlan"),
    ("Vehicles", "Járművek"),
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
    ("Host game", "Játék létrehozása"),
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
    ("host", "a házigazda"),
    ("Space battle formations:", "Űrcsata-alakzatok:"),
    ("off (classic)", "ki (klasszikus)"),
    ("every player picks", "mindenki maga választ"),
    ("Defeated players:", "Legyőzött játékosok:"),
    ("leave (classic)", "kilépnek (klasszikus)"),
    ("stay and watch", "maradnak és néznek"),
    ("Computer empires:", "Gépi birodalmak:"),
    ("classic", "klasszikus"),
    ("enhanced", "bővített"),
    ("Two empires are needed to start", "Az indításhoz két birodalom kell"),
    ("Not everyone is ready: press again to start anyway", "Nem áll mindenki készen: nyomd meg újra az indításhoz"),
    ("Start anyway", "Indítás így is"),
    ("Not ready", "Nem kész"),
    ("Host", "Házigazda"),
    ("Saved game", "Mentett játék"),
    ("LAN games", "LAN-játékok"),
    ("No multiplayer saves", "Nincs többjátékos mentés"),
    ("Pick one of the saved empires", "Válassz egyet a mentett birodalmak közül"),
    ("Port mapping: asking the router...", "Porttovábbítás: kérés a routertől..."),
    ("Port mapping: on, reachable at {}", "Porttovábbítás: be, elérhető itt: {}"),
    ("Port mapping failed: {}", "Porttovábbítás sikertelen: {}"),
    ("Join code: none (no network address)", "Csatlakozási kód: nincs (nincs hálózati cím)"),
    ("Join code: {}  (click: copy)", "Csatlakozási kód: {}  (kattintás: másolás)"),
    ("The join code is on the clipboard", "A csatlakozási kód a vágólapon van"),
    ("Address or join code:", "Cím vagy csatlakozási kód:"),
    ("Address", "Cím"),
    ("Relay server: {}", "Közvetítő szerver: {}"),
    ("Relay server: opening the room...", "Közvetítő szerver: szoba nyitása..."),
    ("Relay server: room open, seen at {}", "Közvetítő szerver: a szoba nyitva, itt látható: {}"),
    ("Relay server: full, try later", "Közvetítő szerver: megtelt, próbáld később"),
    ("Relay server: refused the room", "Közvetítő szerver: elutasította a szobát"),
    ("Relay server: no answer", "Közvetítő szerver: nem válaszol"),
    // The public game list on the relay server (ADR 0060).
    ("Refresh", "Frissítés"),
    ("Asking the relay server...", "Kérdezem a közvetítő szervert..."),
    ("No public games on the relay server", "Nincs nyilvános játék a közvetítő szerveren"),
    ("Type a relay server to browse internet games", "Az internetes játékokhoz adj meg közvetítő szervert"),
    // The multiplayer pages (ADR 0064).
    ("Playing as", "Játékos:"),
    ("Internet games", "Internetes játékok"),
    ("Saved games", "Mentett játékok"),
    ("Join directly", "Közvetlen csatlakozás"),
    ("Relay server", "Közvetítő szerver"),
    ("Optional. For internet games: it lists public games, helps players behind a router reach each other and relays when nothing else works.", "Nem kötelező. Internetes játékhoz: listázza a nyilvános játékokat, segít a router mögötti játékosoknak elérni egymást, és továbbít, ha más nem működik."),
    ("Games hosted on this network show here as they open.", "A hálózaton indított játékok itt jelennek meg."),
    ("Host one yourself, or press Refresh to ask again.", "Indíts egyet magad, vagy kérdezz újra a Frissítéssel."),
    ("A multiplayer game the host saved can be resumed here.", "A házigazda által mentett többjátékos játék itt folytatható."),
    ("Host saved game", "Mentett játék indítása"),
    ("Started", "Elindult"),
    ("Incompatible", "Nem kompatibilis"),
    ("Password", "Jelszó"),
    ("Joined", "Csatlakozva"),
    ("Private", "Zárt"),
    ("Ready", "Kész"),
    ("Players", "Játékosok"),
    ("Connection", "Kapcsolat"),
    ("Port mapping (UPnP / NAT-PMP)", "Porttovábbítás (UPnP / NAT-PMP)"),
    ("Private game", "Zárt játék"),
    ("Joined with the join code only", "Csak csatlakozási kóddal érhető el"),
    ("Public list", "Nyilvános lista"),
    ("Copy join code", "Kód másolása"),
    ("Send it to the players: they paste it into Join directly (Ctrl+V).", "Küldd el a játékosoknak: a Közvetlen csatlakozásba illesztik be (Ctrl+V)."),
    ("Open", "Szabad"),
    ("You", "Te"),
    ("Take seat", "Beülök"),
    ("Type a message; Enter sends it", "Írj üzenetet; Enter elküldi"),
    ("Can't host: {}", "Nem indítható: {}"),
    ("Starting story set to {}", "Kezdő történet: {}"),
    ("Another game version", "Más játékverzió"),
    ("Another game install (rules differ)", "Más telepítés (eltérő szabályok)"),
    ("Other mods enabled (mods differ; see Mods)", "Más modok bekapcsolva (eltérő modok; lásd Modok)"),
    ("This game uses other mods", "Ez a játék más modokat használ"),
    ("Missing:", "Hiányzik:"),
    ("Another version (yours / the game's):", "Más verzió (tiéd / a játéké):"),
    ("Not in this game, switch them off in Mods:", "Nincs ebben a játékban, kapcsold ki a Modokban:"),
    ("Not in the mod index, install them by hand:", "Nincs a modlistában, telepítsd kézzel:"),
    ("The same mods in another order.", "Ugyanazok a modok más sorrendben."),
    ("The same mods with other files: compare them in Mods.", "Ugyanazok a modok más fájlokkal: hasonlítsd össze a Modokban."),
    ("Looking them up in the mod index (github.com/openIG2-org/mods)...", "Keresés a modlistában (github.com/openIG2-org/mods)..."),
    ("The mod index didn't load:", "A modlista nem töltődött be:"),
    ("Downloading the game's mods...", "A játék modjainak letöltése..."),
    ("The game then starts again with these mods and joins.", "A játék ezután újraindul ezekkel a modokkal és csatlakozik."),
    ("Download and join", "Letöltés és csatlakozás"),
    ("Restart and join", "Újraindítás és csatlakozás"),
    ("Open Mods", "Modok megnyitása"),
    ("Wrong join code", "Hibás csatlakozási kód"),
    ("The game is full", "A játék megtelt"),
    ("The game has started", "A játék már elindult"),
    ("Wrong password", "Hibás jelszó"),
    ("Removed by the host", "A házigazda eltávolított"),
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
    ("This game was out of step with the host and has been resynchronised.", "Ez a játék elcsúszott a házigazdáétól, újraszinkronizálva."),
    ("The host has left the game. {} hosts it now.", "A házigazda kilépett. Mostantól {} a házigazda."),
    ("The host has kicked you from the game.", "A házigazda kizárt a játékból."),
    ("The host has left the game.", "A házigazda kilépett a játékból."),
    ("Your empire has fallen. You can watch the game until you leave it.", "A birodalmad elbukott. Kilépésig nézheted a játékot."),
    ("Waiting for the other players", "Várakozás a többi játékosra"),
    // Saves.
    ("Cannot save: {}", "Nem sikerült menteni: {}"),
    ("Quicksave {}", "Gyorsmentés {}"),
    ("Quick saved.", "Gyorsmentés kész."),
    ("Quick loaded.", "Gyorsbetöltés kész."),
    ("Autosave {}", "Automentés {}"),
    ("Voice volume", "Beszéd hangereje"),
    ("Extra game speeds", "Extra játéksebességek"),
    ("Same as sound", "Mint a hang"),
    ("Sound", "Hang"),
    ("CONTROLS", "IRÁNYÍTÁS"),
    ("Reset all keys", "Összes billentyű visszaállítása"),
    ("Original keys", "Eredeti billentyűk"),
    ("Reset", "Visszaállítás"),
    ("Also the key of:", "Ez a billentyű ehhez is tartozik:"),
    ("Press the new key; Esc keeps the old one.", "Nyomd meg az új billentyűt; Esc: marad a régi."),
    ("The loudness of the sound effects, and of speech at Same as sound.", "A hangeffektusok hangereje, és a beszédé, ha Mint a hang."),
    ("The loudness of the advisors' and messages' speech.", "A tanácsadók és az üzenetek beszédének hangereje."),
    ("The fastest speed's key or button pressed again goes faster: x2, then x4.", "A leggyorsabb sebesség gombja újra megnyomva gyorsít: x2, majd x4."),
    ("Enter or a click, then the new key; Esc keeps the old one.", "Enter vagy kattintás, aztán az új billentyű; Esc: marad a régi."),
    ("Every key back to the original game's.", "Minden billentyű az eredeti játékéra."),
    ("Everywhere", "Mindenhol"),
    ("Colony", "Kolónia"),
    ("Design", "Tervezés"),
    ("Messages", "Üzenetek"),
    ("Space battle", "Űrcsata"),
    ("Help", "Súgó"),
    ("Starmap screen", "Csillagtérkép képernyő"),
    ("Colony screen", "Kolónia képernyő"),
    ("Research screen", "Kutatás képernyő"),
    ("Design screen", "Tervezés képernyő"),
    ("Diplomacy screen", "Diplomácia képernyő"),
    ("Spy screen", "Kémkedés képernyő"),
    ("Trade screen", "Kereskedelem képernyő"),
    ("Info screen", "Információ képernyő"),
    ("Messages screen", "Üzenetek képernyő"),
    ("Messages (second key)", "Üzenetek (második billentyű)"),
    ("Options", "Beállítások"),
    ("Pause", "Szünet"),
    ("Pause (second key)", "Szünet (második billentyű)"),
    ("Normal speed", "Normál sebesség"),
    ("Fast speed", "Gyors sebesség"),
    ("Fastest speed", "Leggyorsabb sebesség"),
    ("Quick save (Shift: load)", "Gyorsmentés (Shift: betöltés)"),
    ("Chat", "Csevegés"),
    ("Next planet or fleet", "Következő bolygó vagy flotta"),
    ("Previous planet or fleet", "Előző bolygó vagy flotta"),
    ("Colonise", "Gyarmatosítás"),
    ("Zoom in", "Közelítés"),
    ("Zoom out", "Távolítás"),
    ("Camera up", "Kamera fel"),
    ("Camera left", "Kamera balra"),
    ("Camera down", "Kamera le"),
    ("Camera right", "Kamera jobbra"),
    ("Next planet", "Következő bolygó"),
    ("Previous planet", "Előző bolygó"),
    ("Camera views", "Kameranézetek"),
    ("Planet info", "Bolygóinformáció"),
    ("Building on or off", "Épület be vagy ki"),
    ("Select all units", "Minden egység kijelölése"),
    ("Stop units", "Egységek megállítása"),
    ("Next category", "Következő kategória"),
    ("Previous category", "Előző kategória"),
    ("Redesign", "Áttervezés"),
    ("First answer", "Első válasz"),
    ("Second answer", "Második válasz"),
    ("Third answer", "Harmadik válasz"),
    ("Fourth answer", "Negyedik válasz"),
    ("Select the fleet", "A flotta kijelölése"),
    ("Chase camera", "Követő kamera"),
    // ADR 0080: Discord Rich Presence (the option, the activity's texts, the room's lines).
    ("Discord status", "Discord állapot"),
    ("With join", "Belépéssel"),
    ("Scenario", "Forgatókönyv"),
    ("Race {}", "{}. faj"),
    ("Playing as {}", "Faj: {}"),
    ("Multiplayer room: {}", "Többjátékos szoba: {}"),
    ("Resuming a saved game: {}", "Mentett játék folytatása: {}"),
    ("Waiting for players", "Játékosokra vár"),
    ("Ready to start", "Indításra kész"),
    ("Getting ready", "Készülődik"),
    ("Multiplayer room", "Többjátékos szoba"),
    ("Watching a replay", "Visszajátszást néz"),
    ("Watching the galaxy", "A galaxist figyeli"),
    ("Paused", "Szünetel"),
    ("{} planets", "{} bolygó"),
    ("{} population", "{} lakos"),
    ("{} ships", "{} hajó"),
    ("{} credits", "{} kredit"),
    ("score {}", "pontszám: {}"),
    ("{} techs", "{} technológia"),
    ("{} battles won", "{} megnyert csata"),
    ("{} players", "{} játékos"),
    ("Get openIG2", "openIG2 letöltése"),
    ("Website", "Weboldal"),
    ("On the starmap", "A csillagtérképen"),
    ("Building a colony", "Kolóniát épít"),
    ("Fighting a ground war", "Földi harcot vív"),
    ("In a space battle", "Űrcsatában"),
    ("Researching", "Kutat"),
    ("Designing ships", "Hajókat tervez"),
    ("Negotiating", "Tárgyal"),
    ("Planning spy missions", "Kémakciókat tervez"),
    ("Trading", "Kereskedik"),
    ("Studying the empires", "A birodalmakat tanulmányozza"),
    ("Reading messages", "Üzeneteket olvas"),
    ("In the settings", "A beállításokban"),
    ("Reading the help", "A súgót olvassa"),
    ("In the main menu", "A főmenüben"),
    ("Choosing an empire", "Birodalmat választ"),
    ("Choosing a scenario", "Forgatókönyvet választ"),
    ("Looking for a multiplayer game", "Többjátékos játékot keres"),
    ("Looking at the match history", "A játszmatörténetet nézi"),
    ("Browsing replays", "Visszajátszásokat böngész"),
    ("Managing mods", "Modokat kezel"),
    ("Joining through Discord...", "Csatlakozás Discordon keresztül..."),
    ("is joining through Discord", "csatlakozik Discordon keresztül"),
    ("Shows on your Discord profile what you play; with join, friends can join your multiplayer room from Discord.", "A Discord-profilodon látszik, mivel játszol; belépéssel a barátaid Discordról beléphetnek a többjátékos szobádba."),
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
            let labels = r.labels.into_iter().map(|l| String::from_utf8(l).unwrap()).filter(|l| !l.contains('x') && !l.ends_with('%') && !langs.contains(&l.as_str()));
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

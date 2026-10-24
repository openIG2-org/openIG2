//! The starmap's hot-spot tooltips (re/screens/input.md §8): `Starmap_MouseInit` (exe 0x5b99d0) gives each hot-spot
//! its text `GetString(2, 1, n)`, `Starmap_MouseInit2` (exe 0x5b8950) switches them per mode and rebuilds the info
//! box's (8) and the exit's (9); with the radial menu open the icon under the pointer shows its name at once
//! (`StarmapRopIkon_Nev`, `UpdateFrame` exe 0x4e3e90).

/// What the starmap's tooltip shows: the hot-spot (its id), the text and whether it shows at once (the radial icons).
pub struct StarmapTip {
    pub id: u32,
    pub text: Vec<u8>,
    pub now: bool,
}

/// The starmap's state the tooltip reads.
pub struct Ctx<'a> {
    pub texts: &'a crate::ui::UiTexts,
    pub state: &'a ig2_sim::save::SaveState,
    pub sel: crate::orders::Selection,
    pub menus: &'a crate::starmap_menus::Menus,
    pub panels: &'a crate::starmap_panels::Panels,
    /// The console's or minimap's hot-spot (1..=7) under the pointer, while they are on.
    pub console: Option<u32>,
    /// `TutorialStarmapMindenLetiltva…`: only 1..=7.
    pub locked: bool,
    pub human: u8,
}

/// The starmap's tooltip at the virtual point `at`: the lowest enabled hot-spot under it and its text.
pub fn tip(c: &Ctx, at: [f32; 2]) -> Option<StarmapTip> {
    use crate::starmap_info::Kind;
    let line = |n: u16| c.texts.get(2, 1, n);
    // The radial menu's icon under the pointer (`AktRopIkon2 > 0 && StarmapRopIkonMod`), on hot-spot 0x35.
    if let Some(r) = c.menus.rop.filter(|r| r.hover > 0) {
        return Some(StarmapTip { id: 0x35, text: c.texts.get(17, 6, r.hover as u16), now: true });
    }
    let (state, sel, p) = (c.state, c.sel, c.panels);
    let long = c.menus.info_on();
    let kind = p.info.as_ref().map(|i| i.kind);
    let others = if c.locked {
        None
    } else if long {
        // The long info: 8 and 9, its own buttons and stations; 1..=7 and 0x35 off.
        let exit = crate::starmap_menus::exit_spot(at).then_some(9);
        let boxed = crate::kit::inside_f([0.0, 424.0, 166.0, 56.0], at).then_some(8);
        [exit, boxed, c.menus.info_spot(state, at)].into_iter().flatten().min()
    } else {
        let (tanks, product) = crate::starmap_panels::Panels::buttons(state, &sel);
        let boxed = crate::kit::inside_f([0.0, 424.0, 166.0, 56.0], at).then_some(8);
        let panel = p.spot(state, &sel, at).filter(|&id| match id {
            10 => tanks,
            11 => product,
            _ => true,
        });
        [boxed, panel].into_iter().flatten().min()
    };
    let id = [c.console, others].into_iter().flatten().min()?;
    let human = c.human;
    let fleet = state.fleets.get(sel.fleet);
    let own_fleet = fleet.is_some_and(|f| f[0x15] == human);
    let n = match id {
        1..=7 | 0xa..=0xd => id as u16,
        8 => {
            // `Starmap_MouseInit2`'s info box text, then in single player "  " and (2, 1, 0x2f) (`strncat` to 0x50).
            let base = match sel.mode {
                1 => Some(if long { 0x2a } else { 0x29 }),
                2 if kind == Some(Kind::Fleet) => Some(0x2b),
                2 if own_fleet && p.transfer.is_some() => Some(0x2c),
                2 if own_fleet => Some(if fleet.is_some_and(|f| f[0x52] & 1 == 0) { 0x2d } else { 0x2e }),
                _ => None,
            };
            let mut t: Vec<u8> = base.map(|n| line(n).into_iter().take_while(|&b| b != 0).collect()).unwrap_or_default();
            let suffix = state.mem.u8(ig2_sim::save::va::MULTIPLAYER) == 0 && ((sel.mode == 1 && !long) || (sel.mode == 2 && own_fleet));
            if suffix {
                t.extend_from_slice(b"  ");
                t.extend(line(0x2f).into_iter().take_while(|&b| b != 0));
                t.truncate(0x50);
            }
            return Some(StarmapTip { id, text: t, now: false });
        }
        // The exit: its text per mode.
        9 => match () {
            _ if long || kind == Some(Kind::Tanks) => 0x28,
            _ if kind == Some(Kind::Fleet) => 0x26,
            _ if p.transfer.is_some() => 0x24,
            _ if p.product || (p.p_moving && p.p_opening) => 0x23,
            _ => 9,
        },
        0xe..=0x15 => 0xe,
        0x16..=0x19 => 0xf,
        0x1a..=0x21 => 0x10,
        0x22..=0x29 => 0x11,
        0x2a..=0x31 => 0x12,
        0x32 => 0x13,
        0x36 => 0x14,
        // The panels' cells: the vehicles and the fleet's 0x27, the transfer's 0x25.
        0x43..=0x52 if kind.is_some() => 0x27,
        0x43..=0x52 if p.transfer.is_some() => 0x25,
        0x43..=0x52 => 0x15,
        0x53 | 0x55 => 0x16,
        0x54 | 0x56 => 0x17,
        0x57 => 0x18,
        0x5a..=0x63 => id as u16 - 0x41,
        _ => return Some(StarmapTip { id, text: Vec::new(), now: false }),
    };
    Some(StarmapTip { id, text: line(n), now: false })
}

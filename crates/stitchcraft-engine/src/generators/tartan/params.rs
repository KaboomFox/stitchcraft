//! A tartan fill's settings, under Ink/Stitch's names and with the defaults it sews for a file that sets
//! none, which its files and its tartan fonts count on (`docs/src/design/algorithms/tartan.md`).
//!
//! **Why here.** A setting is declared once, beside the generator that reads it, and the registry, the
//! reference pages, the JSON Schema, VectorCraft's dialogs and the compatibility contract are made from the
//! declaration. Reading them into what the generator needs ([`TartanParams::tartan`]) is here too, so that
//! what a setting means, and what is said when it can't be sewn as set, lies beside it.
//!
//! **What changes as they are read.** A sett that can't be read skips the element (`SC-E0323`), and an empty
//! one sews Ink/Stitch's default (`SC-W0325`). A colour that can't be read leaves its stripe unsewn
//! (`SC-W0328`), a sett with no stripe to sew sews its underlay alone (`SC-W0326`), and one whose stripes are
//! too narrow for the shortest stitch loses needle points (`SC-W0327`). A herringbone sews from M10.7, and
//! until then the rows are sewn without it (`SC-W0324`).

use stitchcraft_core::math::{cos, sin, to_radians};
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Mm};
use stitchcraft_params::{StitchType, params};
use stitchcraft_plan::Rgb;

use super::Tartan;
use super::sett::{Item, read};
use crate::generators::mm;
use crate::generators::tatami::sew::{Layer, Travel};

/// The stitch types a tartan fill's own settings apply to.
const TARTAN: &[StitchType] = &[StitchType::TartanFill];

/// The stitch types whose colours can travel on to the fill's end: a tartan fill's, and a linear gradient
/// fill's, which reads no other setting of its own.
const STOPPING: &[StitchType] = &[StitchType::LinearGradientFill, StitchType::TartanFill];

params! {
    /// Tartan fill: rows of running stitches across the fill, a twill of warp and weft that weaves a plaid.
    /// The sett gives the stripes, each a colour and a width, and the rows go to the warp's stripes and the
    /// weft's in turns. Each colour's rows are sewn in its own thread, after the fill's underlay, and the
    /// stripes too narrow for rows are sewn as runs along their middles last.
    pub struct TartanParams for TARTAN;

    "Tartan" {
        /// The sett, as Ink/Stitch's tartan editor saves it: an object of JSON whose `palette` gives the
        /// stripes, each a colour in brackets and its width in millimetres, such as `(#0000ff)/2.4
        /// (#ffffff)0.4 (#ff0000)/2.4`. A `?` before a width leaves its stripe unsewn, a `*` sews it in the
        /// colour of the stripe before it, and a `|` parts the warp's stripes from the weft's. `symmetry`
        /// mirrors the sett at its ends rather than repeating it, `scale` scales it in percent, `rotate` turns
        /// it in degrees, `offset_x` and `offset_y` move it in millimetres, and stripes no wider than
        /// `min_stripe_width` sew as runs. Empty, the fill sews Ink/Stitch's default sett. VectorCraft's fill
        /// dialog doesn't hold a sett before M10.7.
        tartan: Text = "", label "Sett";

        /// The angle of the rows to the stripes, in degrees: at -45 they slant across both the warp's and the
        /// weft's stripes, as a woven twill's do. At 0 or 90 they run along one direction's stripes.
        tartan_angle: Angle = "-45", label "Row angle";

        /// How many rows in a row go to the warp's stripes, before as many go to the weft's. More rows to a
        /// thread make the weave coarser, as a heavier twill.
        rows_per_thread: Count = "2", label "Rows per thread", range (1, 100);

        /// The width of the bands of a herringbone, each band's rows slanting the other way from the band
        /// before. 0 sews straight rows. StitchCraft sews herringbones from M10.7, and until then the rows are
        /// sewn without them, which `SC-W0324` says.
        herringbone_width_mm: Length = "0", label "Herringbone width", range (0.0, 100.0);
    }

    "Colours" {
        /// End the fill's colours at its end position, or near the next element. A tartan fill's every colour
        /// then travels from where the fill starts to where it ends, and a linear gradient fill's last 2
        /// colours travel on to the end after their rows. Off, each colour ends with its last row, and a
        /// tartan starts each colour where the one before ended, and only its runs travel on towards its end.
        stop_at_ending_point: Toggle = "false", label "Stop at the end position", applies STOPPING;
    }
}

/// What a tartan's own settings are read with: the tatami fill's rows and underlay, already read.
pub(crate) struct Given<'g> {
    /// The rows' spacing and staggers, its `skip_last`, its underlay's layers, inset and expand.
    pub(crate) spacing: f64,
    pub(crate) staggers: f64,
    pub(crate) skip_last: bool,
    pub(crate) underlay: Vec<Layer>,
    pub(crate) inset: f64,
    pub(crate) expand: f64,
    /// The rows' longest stitch, and the needle's travel in the running stitch's length.
    pub(crate) longest: Mm,
    pub(crate) travel: Travel,
    /// The runs' bean stitch.
    pub(crate) beans: &'g [u32],
    /// The element's shortest stitch, and its own thread, which its underlay sews in.
    pub(crate) min_stitch: Mm,
    pub(crate) own: Rgb,
}

impl TartanParams {
    /// The tartan these settings sew with `given`, what is said of them in `diagnostics`; `None`, with
    /// `SC-E0323`, when its sett can't be read.
    pub(crate) fn tartan(&self, given: Given<'_>, diagnostics: &mut Vec<Diagnostic>) -> Option<Tartan> {
        let read = match read(&self.tartan) {
            Ok(read) => read,
            Err(why) => {
                let message = format!("This tartan fill's sett can't be read, as {why}, so it is skipped.");
                diagnostics.push(Diagnostic::new(Code::FillTartanSettUnreadable, message));
                return None;
            }
        };
        if self.tartan.trim().is_empty() {
            let message = "This tartan fill has no sett, so it sews Ink/Stitch's default one, stripes 5 mm wide of a thread almost black, as \
                           Ink/Stitch does.";
            diagnostics.push(Diagnostic::new(Code::FillTartanNoSett, message));
        }
        if !read.unread.is_empty() {
            let names: Vec<String> = read.unread.iter().map(|name| format!("`{name}`")).collect();
            let (colours, their) = if names.len() == 1 { ("a colour", "its") } else { ("colours", "their") };
            let message = format!(
                "This tartan's sett names {} as {colours}, which can't be read, so {their} stripes are left unsewn, as Ink/Stitch leaves them.",
                names.join(" and ")
            );
            diagnostics.push(Diagnostic::new(Code::FillTartanColourUnreadable, message));
        }
        if self.herringbone_width_mm.get() > 0.0 {
            let message = format!(
                "This tartan's herringbone is {} mm wide, and StitchCraft sews herringbones from M10.7, so its rows are sewn without one.",
                mm(self.herringbone_width_mm.get())
            );
            diagnostics.push(Diagnostic::new(Code::FillTartanHerringboneNotYet, message));
        }
        let sett = read.sett;
        let (warp, weft) = (sett.full(&sett.warp), sett.full(&sett.weft));
        if warp.is_empty() && weft.is_empty() {
            let message = "This tartan's sett sews none of its stripes, so only the fill's underlay is sewn, as in Ink/Stitch.";
            diagnostics.push(Diagnostic::new(Code::FillTartanNoStripe, message));
        }
        diagnostics.extend(too_fine(&warp, &weft, self.tartan_angle, given.min_stitch.get()));
        let Given { spacing, staggers, skip_last, underlay, inset, expand, longest, travel, beans, own, .. } = given;
        Some(Tartan {
            margin: sett.margin,
            sett,
            warp,
            weft,
            angle: self.tartan_angle,
            spacing,
            longest: longest.get(),
            staggers,
            rows_per_thread: self.rows_per_thread.max(1),
            run_length: travel.length,
            beans: beans.to_vec(),
            skip_last,
            stop_at_end: self.stop_at_ending_point,
            underlay,
            inset,
            expand,
            travel,
            own,
        })
    }
}

/// `SC-W0327` when the rows at `angle` to the stripes cross the narrowest band of `warp` or `weft` sewn as a
/// fill in a stretch shorter than `min_stitch`: a warp band in its width over the cosine of the angle, and a
/// weft band over its sine. Bands of a colour that meet are one, as the fill lays them.
fn too_fine(warp: &[Item], weft: &[Item], angle: f64, min_stitch: f64) -> Option<Diagnostic> {
    let radians = to_radians(angle);
    let slant = |items: &[Item], across: f64| narrowest(items).map(|width| (width, width / across.abs()));
    let crossed = [slant(warp, cos(radians)), slant(weft, sin(radians))];
    let (width, stretch) = crossed.into_iter().flatten().filter(|(_, stretch)| stretch.is_finite()).min_by(|a, b| a.1.total_cmp(&b.1))?;
    if at_least(stretch, min_stitch) {
        return None;
    }
    let message = format!(
        "This tartan's narrowest stripe sewn as a fill is {} mm wide, so each row crosses it in {} mm, shorter than the shortest stitch ({} mm), \
         and needle points of its stripes are left out.",
        mm(width),
        mm(stretch),
        mm(min_stitch)
    );
    Some(Diagnostic::new(Code::FillTartanTooFine, message))
}

/// The width of the narrowest band `items` sew as a fill, a band of a colour joined to the next of that colour
/// when nothing wider than a run lies between, round the end of the sett as it repeats; `None` for none.
fn narrowest(items: &[Item]) -> Option<f64> {
    // A band left unsewn parts the bands beside it, and measures as no band.
    let mut bands: Vec<(Option<Rgb>, f64)> = Vec::new();
    for item in items {
        match *item {
            Item::Band { colour: Some(colour), width } => match bands.last_mut() {
                Some((Some(last), total)) if *last == colour => *total += width,
                _ => bands.push((Some(colour), width)),
            },
            Item::Band { colour: None, width } if width > 0.0 => bands.push((None, f64::INFINITY)),
            Item::Band { .. } | Item::Run(_) => {}
        }
    }
    // The sett repeats: a last band of the first band's colour is one band with it.
    if bands.len() > 1
        && let (Some(&(first, a)), Some(&(last, b))) = (bands.first(), bands.last())
        && first == last
        && first.is_some()
    {
        bands.pop();
        if let Some(head) = bands.first_mut() {
            head.1 = a + b;
        }
    }
    bands.iter().filter(|(colour, _)| colour.is_some()).map(|(_, width)| *width).filter(|width| *width > 0.0).min_by(f64::total_cmp)
}

#[cfg(test)]
mod tests {
    use stitchcraft_params::ParamSet;

    use super::*;

    const RED: Rgb = Rgb::new(255, 0, 0);
    const BLUE: Rgb = Rgb::new(0, 0, 255);
    const TRAVEL: Travel = Travel { length: 2.5, tolerance: 0.2, min_stitch: 0.3, underpath: false, jump: 3.0 };

    fn band(colour: Rgb, width: f64) -> Item {
        Item::Band { colour: Some(colour), width }
    }

    /// The tartan `set` gives, read with a shortest stitch of `min_stitch` mm, and the codes it says.
    fn read(set: &[(&str, &str)], min_stitch: f64) -> (Option<Tartan>, Vec<Code>) {
        let set: ParamSet = set.iter().copied().collect();
        let params = TartanParams::from_set_as(&set, StitchType::TartanFill).unwrap().params;
        let given = Given {
            spacing: 0.25,
            staggers: 4.0,
            skip_last: true,
            underlay: Vec::new(),
            inset: 0.5,
            expand: 0.25,
            longest: Mm::new(3.0).unwrap(),
            travel: TRAVEL,
            beans: &[1, 0],
            min_stitch: Mm::new(min_stitch).unwrap(),
            own: Rgb::new(1, 2, 3),
        };
        let mut said = Vec::new();
        let tartan = params.tartan(given, &mut said);
        (tartan, said.iter().map(|d| d.code).collect())
    }

    #[test]
    fn a_tartan_takes_its_own_settings_and_what_it_is_given() {
        let sett = r#"{"palette": "(#ff0000)/5 (#0000ff)/5"}"#;
        let (tartan, said) = read(&[("tartan", sett), ("tartan_angle", "-30"), ("rows_per_thread", "3"), ("stop_at_ending_point", "true")], 0.3);
        let tartan = tartan.unwrap();
        assert_eq!(said, []);
        assert_eq!((tartan.angle, tartan.rows_per_thread, tartan.stop_at_end), (-30.0, 3, true));
        assert_eq!((tartan.spacing, tartan.staggers, tartan.longest, tartan.run_length), (0.25, 4.0, 3.0, 2.5));
        assert_eq!((tartan.skip_last, tartan.inset, tartan.expand, tartan.beans.as_slice()), (true, 0.5, 0.25, &[1, 0][..]));
        assert_eq!((tartan.margin, tartan.own, tartan.travel), (10.0, Rgb::new(1, 2, 3), TRAVEL));
        // The full setts: mirrored, so red and blue once each.
        assert_eq!(tartan.warp, [band(RED, 5.0), band(BLUE, 5.0)]);
        assert_eq!(tartan.weft, tartan.warp);
    }

    #[test]
    fn diag_sc_e0323_a_sett_that_can_t_be_read_skips_the_element() {
        let (tartan, said) = read(&[("tartan", "{")], 0.3);
        assert_eq!((tartan, said), (None, vec![Code::FillTartanSettUnreadable]));
    }

    #[test]
    fn diag_sc_w0325_an_empty_sett_sews_ink_stitch_s_default() {
        let (tartan, said) = read(&[], 0.3);
        assert_eq!(said, [Code::FillTartanNoSett]);
        let near_black = Rgb::new(0x10, 0x10, 0x10);
        assert_eq!(tartan.unwrap().warp, [band(near_black, 5.0), Item::Band { colour: None, width: 5.0 }]);
    }

    #[test]
    fn diag_sc_w0328_unread_colours_are_named_once_or_together() {
        let (_, said) = read(&[("tartan", r#"{"palette": "(nocolour)/5 (#0000ff)/5"}"#)], 0.3);
        assert_eq!(said, [Code::FillTartanColourUnreadable]);
        let message = |sett: &str| {
            let set: ParamSet = [("tartan", sett)].into_iter().collect();
            let params = TartanParams::from_set_as(&set, StitchType::TartanFill).unwrap().params;
            let given = Given {
                spacing: 0.25,
                staggers: 4.0,
                skip_last: false,
                underlay: Vec::new(),
                inset: 0.0,
                expand: 0.0,
                longest: Mm::new(3.0).unwrap(),
                travel: TRAVEL,
                beans: &[0],
                min_stitch: Mm::new(0.3).unwrap(),
                own: Rgb::new(0, 0, 0),
            };
            let mut said = Vec::new();
            params.tartan(given, &mut said);
            said.into_iter().map(|d| d.message).collect::<Vec<_>>()
        };
        assert_eq!(
            message(r#"{"palette": "(nocolour)/5 (#0000ff)/5"}"#),
            ["This tartan's sett names `nocolour` as a colour, which can't be read, so its stripes are left unsewn, as Ink/Stitch leaves them."]
        );
        assert_eq!(
            message(r#"{"palette": "(one)/5 (#0000ff)5 (two)/5"}"#),
            [
                "This tartan's sett names `one` and `two` as colours, which can't be read, so their stripes are left unsewn, as Ink/Stitch leaves them."
            ]
        );
    }

    #[test]
    fn diag_sc_w0324_a_herringbone_is_said_and_sewn_straight() {
        let (tartan, said) = read(&[("tartan", r#"{"palette": "(#ff0000)/5 (#0000ff)/5"}"#), ("herringbone_width_mm", "3")], 0.3);
        assert_eq!(said, [Code::FillTartanHerringboneNotYet]);
        assert!(tartan.is_some());
    }

    #[test]
    fn diag_sc_w0326_a_sett_with_no_stripe_to_sew_sews_its_underlay_alone() {
        let (tartan, said) = read(&[("tartan", r#"{"palette": "(#ff0000)?5 (#0000ff)?5"}"#)], 0.3);
        assert_eq!(said, [Code::FillTartanNoStripe]);
        let tartan = tartan.unwrap();
        assert!(tartan.warp.is_empty() && tartan.weft.is_empty());
        // A weft that sews nothing leaves the warp's stripes, and nothing to say.
        let (tartan, said) = read(&[("tartan", r#"{"palette": "(#ff0000)/5 | (#0000ff)?5"}"#)], 0.3);
        assert_eq!(said, []);
        let tartan = tartan.unwrap();
        assert!(!tartan.warp.is_empty() && tartan.weft.is_empty());
    }

    #[test]
    fn diag_sc_w0327_rows_crossing_a_stripe_in_less_than_the_shortest_stitch_say_so() {
        // Stripes 1.5 mm wide, wider than the narrowest of 1 mm, crossed at 45° in 2.12 mm.
        let sett = r#"{"palette": "(#ff0000)/1.5 (#0000ff)/1.5"}"#;
        assert_eq!(read(&[("tartan", sett)], 2.0).1, []);
        assert_eq!(read(&[("tartan", sett)], 2.2).1, [Code::FillTartanTooFine]);
        let message = too_fine(&[band(RED, 1.5), band(BLUE, 1.5)], &[], -45.0, 2.2).unwrap().message;
        assert_eq!(
            message,
            "This tartan's narrowest stripe sewn as a fill is 1.5 mm wide, so each row crosses it in 2.12 mm, shorter than the shortest stitch \
             (2.2 mm), and needle points of its stripes are left out."
        );
        // Rows along the warp's stripes never cross them, and the weft's are crossed square to them.
        assert!(too_fine(&[band(RED, 1.5)], &[band(BLUE, 1.5)], 90.0, 1.5).is_none());
        assert!(too_fine(&[band(RED, 1.5)], &[band(BLUE, 1.5)], 90.0, 1.6).is_some());
        // Nothing sewn as a fill, nothing to say.
        assert!(too_fine(&[Item::Run(RED)], &[], -45.0, 10.0).is_none());
    }

    #[test]
    fn the_narrowest_band_joins_bands_of_a_colour_across_runs_and_round_the_sett() {
        // Red 2 and red 3 meet across a run: one band of 5. Blue 1 is the narrowest.
        assert_eq!(narrowest(&[band(RED, 2.0), Item::Run(BLUE), band(RED, 3.0), band(BLUE, 1.0)]), Some(1.0));
        // The last red joins the first round the end: 4 + 2, wider than blue's 5.
        assert_eq!(narrowest(&[band(RED, 4.0), band(BLUE, 5.0), band(RED, 2.0)]), Some(5.0));
        // An unsewn band parts 2 reds, and isn't measured; round the end, the last red meets the first.
        assert_eq!(narrowest(&[band(RED, 2.0), Item::Band { colour: None, width: 1.0 }, band(RED, 3.0), band(BLUE, 4.0)]), Some(2.0));
        assert_eq!(narrowest(&[band(RED, 2.0), Item::Band { colour: None, width: 1.0 }, band(RED, 3.0)]), Some(5.0));
        // A band of no width is none, and an unsewn band of no width parts nothing.
        assert_eq!(narrowest(&[band(RED, 2.0), Item::Band { colour: None, width: 0.0 }, band(RED, 0.0)]), Some(2.0));
        assert_eq!(narrowest(&[Item::Band { colour: None, width: 3.0 }]), None);
        assert_eq!(narrowest(&[]), None);
        // 2 reds that meet are one band of 2, wider than either; an unsewn band of no width between them
        // leaves them one; and a red band of no width leaves blue's 5 the narrowest.
        assert_eq!(narrowest(&[band(RED, 1.0), band(RED, 1.0), band(BLUE, 5.0)]), Some(2.0));
        assert_eq!(narrowest(&[band(RED, 1.0), Item::Band { colour: None, width: 0.0 }, band(RED, 1.0), band(BLUE, 5.0)]), Some(2.0));
        assert_eq!(narrowest(&[band(RED, 0.0), band(BLUE, 5.0)]), Some(5.0));
    }
}

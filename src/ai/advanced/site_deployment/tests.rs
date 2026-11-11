use super::*;
use crate::ai::{run_game, Ai};
use crate::game::GameOptions;

fn played(options: &GameOptions, seat: impl Fn() -> AdvancedAi) -> String {
    let mut game = Game::new_with(options.clone());
    let mut ais: Vec<AdvancedAi> = game.players.iter().map(|_| seat()).collect();
    run_game(&mut game, &mut ais);
    serde_json::to_string(&game.log).expect("the action log serializes")
}

#[test]
fn every_site_experiment_is_a_registered_opt_in_named_once() {
    for tag in SITE_EXPERIMENT_GENES {
        let row = gene(tag).unwrap_or_else(|| panic!("`{tag}` is not a registry tag"));
        assert!(
            row.opt_in(),
            "`{tag}` is not an opt-in: the site list only adds to the ledger genome"
        );
        assert_eq!(
            SITE_EXPERIMENT_GENES
                .iter()
                .filter(|other| *other == tag)
                .count(),
            1,
            "`{tag}` is listed twice"
        );
    }
    assert_eq!(site_experiment_genes().count(), SITE_EXPERIMENT_GENES.len());
}

#[test]
fn the_site_player_carries_its_list_on_the_authoritative_board() {
    let mut site = AdvancedAi::new();
    site.enable_site_deployment();
    assert!(
        !site.uses_player_observation(),
        "a site major plans on the authoritative board"
    );
    assert!(site.each_civ_draws_its_width);
    let mut native = AdvancedAi::new();
    native.enable_native_deployment();
    assert!(
        !native.each_civ_draws_its_width,
        "the deployment genome itself is unchanged by the site's list"
    );
}

/// The site adds its experiments to the deployment genome and changes
/// nothing else: with the list's genes taken back off, a site major plays
/// the native deployment's whole decision stream.
#[test]
fn the_site_player_without_its_list_is_the_native_deployment() {
    let options = GameOptions::new(3, 32, 20, 90_913, 30, 1);
    let native = || {
        let mut ai = AdvancedAi::new();
        ai.enable_native_deployment();
        ai
    };
    let site_without_its_list = || {
        let mut ai = AdvancedAi::new();
        ai.enable_site_deployment();
        for gene in site_experiment_genes() {
            (gene.disable)(&mut ai);
        }
        ai
    };
    assert_eq!(
        played(&options, native),
        played(&options, site_without_its_list),
        "the site player is the native deployment plus its list"
    );
}

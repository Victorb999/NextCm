//! O que o app tem aberto: o mundo base (enciclopédia) e, se houver, um jogo em curso.
//! Usado igual pelo Tauri e pela ponte de dev, para os dois nunca divergirem.

use std::path::{Path, PathBuf};

use crate::{
    BoardConfidence, BoardRequest, BoardView, ClubFinances, ClubListItem, ClubProfile, ClubTactics, ContractReply, ContractTerms,
    DisciplineOptions, Error, Fixture, Game, GameInfo, Instructions, JobsView, LiveView, MatchPreview, NewGame, NewsItem, NumberAssign,
    OfferView, PersonProfile, ReportStaff, Result, SaveSummary, ScoutList, SearchQuery, SearchResult, Selection, SquadNumbers, StageView,
    StandingRow, TransferItem, TransferView, World, list_saves,
};

use crate::CareerClub;
use crate::LineupCheck;
use crate::NewsLink;
use crate::PressView;
use crate::WorldTransfers;
use crate::{DealTerms, LoanTerms};

pub struct Session {
    world_path: PathBuf,
    saves_dir: PathBuf,
    world: World,
    game: Option<Game>,
}

impl Session {
    pub fn open(world_path: &Path, saves_dir: &Path) -> Result<Session> {
        Ok(Session {
            world_path: world_path.to_path_buf(),
            saves_dir: saves_dir.to_path_buf(),
            world: World::open(world_path)?,
            game: None,
        })
    }

    /// Banco para as consultas: o do jogo aberto (vê o treinador no clube) ou o mundo base.
    pub fn db(&self) -> &World {
        self.game.as_ref().map_or(&self.world, |g| g.world())
    }

    fn game(&self) -> Result<&Game> {
        self.game.as_ref().ok_or_else(|| Error::Invalid("Nenhum jogo aberto.".into()))
    }

    fn game_mut(&mut self) -> Result<&mut Game> {
        self.game.as_mut().ok_or_else(|| Error::Invalid("Nenhum jogo aberto.".into()))
    }

    /// Pesquisa Jogador & Staff, com os valores na data do jogo.
    pub fn search_people(&self, q: &SearchQuery) -> Result<SearchResult> {
        match &self.game {
            Some(g) => {
                let mut q = q.clone();
                if q.injured.is_some() {
                    q.injured_ids = g.injured_ids();
                }
                let mut r = g.world().search_people_on(&q, g.date())?;
                g.tag_people(&mut r.items)?;
                Ok(r)
            }
            None => self.world.search_people(q),
        }
    }

    /// Perfil com o estado do jogador no jogo em curso (condição, moral, lesão...).
    pub fn get_person(&self, id: i32) -> Result<PersonProfile> {
        let mut p = self.db().get_person(id)?;
        p.status = self.game.as_ref().and_then(|g| g.player_status(id));
        // O valor muda com a idade e o contrato: na data do jogo
        if let (Some(g), Some(player)) = (&self.game, &mut p.player) {
            player.value = g.world().market_value_of(id, g.date())?;
        }
        // Os atributos que o clube do treinador não conhece
        if let Some(g) = &self.game {
            g.mask_profile(&mut p)?;
            g.fill_record(&mut p)?;
        }
        Ok(p)
    }

    /// Clube com a condição e os indisponíveis do plantel no jogo em curso.
    pub fn get_club(&self, id: i32) -> Result<ClubProfile> {
        let mut c = self.db().get_club(id)?;
        if let Some(g) = &self.game {
            for p in &mut c.players {
                if let Some(st) = g.player_status(p.id) {
                    p.condition = Some(st.condition);
                    p.injured = st.injury.is_some();
                    p.suspended = st.suspended > 0;
                    p.reserve = st.reserve;
                }
            }
        }
        Ok(c)
    }

    /// Tácticas de um clube; no jogo em curso, a do treinador é editável.
    pub fn get_club_tactics(&self, id: i32) -> Result<ClubTactics> {
        match &self.game {
            Some(g) => g.club_tactics(id),
            None => self.world.get_club_tactics(id),
        }
    }

    /// Como fica a equipa do treinador com esta escolha (sem gravar).
    pub fn preview_tactics(&self, selection: &Selection) -> Result<ClubTactics> {
        self.game()?.preview_selection(selection)
    }

    /// Põe ou tira um jogador da escolha (sem gravar).
    pub fn toggle_player(&self, selection: &Selection, player: i32) -> Result<ClubTactics> {
        self.game()?.toggle_player(selection, player)
    }

    /// Grava a escolha do treinador.
    pub fn set_tactics(&mut self, selection: Selection) -> Result<ClubTactics> {
        self.game_mut()?.set_selection(selection)
    }

    pub fn game_info(&self) -> Result<Option<GameInfo>> {
        self.game.as_ref().map(Game::info).transpose()
    }

    /// Apaga um jogo gravado (não o que está aberto).
    pub fn delete_save(&mut self, file: &str) -> Result<()> {
        if let Some(g) = &self.game
            && g.path().file_name().and_then(|n| n.to_str()) == Some(file)
        {
            return Err(Error::Invalid("Não é possível apagar o jogo que está aberto.".into()));
        }
        crate::delete_save(&self.saves_dir, file)
    }

    pub fn list_saves(&self) -> Result<Vec<SaveSummary>> {
        list_saves(&self.saves_dir)
    }

    pub fn new_game(&mut self, new: &NewGame) -> Result<GameInfo> {
        // A seed só precisa variar entre jogos; a partir dela tudo é reproduzível
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as i64 & i64::MAX);
        let mut game = Game::create(&self.world_path, &self.saves_dir, new, seed)?;
        game.set_watch(true);
        let info = game.info()?;
        self.game = Some(game);
        Ok(info)
    }

    /// Abre um save da pasta de saves pelo nome do arquivo (nunca um caminho qualquer).
    pub fn load_game(&mut self, file: &str) -> Result<GameInfo> {
        let name = Path::new(file).file_name().ok_or_else(|| Error::Invalid("Arquivo inválido.".into()))?;
        let mut game = Game::load(&self.saves_dir.join(name))?;
        game.set_watch(true);
        game.resume_match_day()?;
        let info = game.info()?;
        self.game = Some(game);
        Ok(info)
    }

    pub fn save_game(&mut self) -> Result<GameInfo> {
        let game = self.game_mut()?;
        game.save()?;
        game.info()
    }

    /// Fecha o jogo sem gravar (as mudanças desde a última gravação perdem-se).
    pub fn close_game(&mut self) {
        self.game = None;
    }

    pub fn advance_day(&mut self) -> Result<GameInfo> {
        let game = self.game_mut()?;
        game.advance_day()?;
        game.info()
    }

    pub fn set_nickname(&mut self, id: i32, nickname: String) -> Result<()> {
        self.game_mut()?.set_nickname(id, &nickname)
    }

    pub fn set_note(&mut self, id: i32, text: String) -> Result<()> {
        self.game_mut()?.set_note(id, &text)
    }

    /// A pergunta antes de dispensar (com a indemnização).
    pub fn release_question(&self, id: i32) -> Result<String> {
        self.game()?.release_question(id)
    }

    pub fn release_player(&mut self, id: i32) -> Result<String> {
        self.game_mut()?.release_player(id)
    }

    /// "Rescindir com Clube".
    pub fn resign(&mut self) -> Result<String> {
        self.game_mut()?.resign()
    }

    /// "Retirar-se": grava e fecha o jogo.
    pub fn retire_manager(&mut self) -> Result<String> {
        let game = self.game_mut()?;
        let text = game.retire_manager()?;
        game.save()?;
        self.game = None;
        Ok(text)
    }

    /// "Ir de Férias" durante `days` dias.
    pub fn holiday(&mut self, days: i32) -> Result<GameInfo> {
        self.game_mut()?.holiday(days)
    }

    /// O Currículo do treinador.
    pub fn manager_career(&self) -> Result<Vec<CareerClub>> {
        self.game()?.manager_career()
    }

    /// "Gravar Jogo" com outro nome: grava, copia para o ficheiro novo e continua nele.
    /// Com um ficheiro desse nome já gravado, só substitui com `overwrite`.
    pub fn save_as(&mut self, name: String, overwrite: bool) -> Result<GameInfo> {
        let stem = crate::game::file_stem(&name)?;
        let game = self.game_mut()?;
        game.save()?;
        let target = self.saves_dir.join(format!("{stem}.sqlite"));
        let current = self.game()?.path().to_path_buf();
        if target != current {
            if target.exists() && !overwrite {
                return Err(Error::Invalid("Ficheiro Existe".into()));
            }
            std::fs::copy(&current, &target)?;
            let copy = rusqlite::Connection::open(&target)?;
            copy.execute("UPDATE game SET name = ?", [name.trim()])?;
            drop(copy);
            self.game = None;
            let mut game = Game::load(&target)?;
            game.set_watch(true);
            game.resume_match_day()?;
            self.game = Some(game);
        }
        self.game()?.info()
    }

    /// "Transferências": os negócios do mundo num mês ("2025-08"), página a página.
    pub fn world_transfers(&self, month: String, page: i64) -> Result<WorldTransfers> {
        self.game()?.world_transfers(&month, page)
    }

    /// As competições com jogos acabados hoje (os resultados depois do jogo do treinador).
    pub fn day_competitions(&self) -> Result<Vec<i32>> {
        self.game()?.day_competitions()
    }

    /// Uma conferência de imprensa: as perguntas e as respostas.
    pub fn press_view(&self, id: i32) -> Result<PressView> {
        self.game()?.press_view(id)
    }

    /// As respostas do treinador na conferência (uma por pergunta).
    pub fn answer_press(&mut self, id: i32, answers: Vec<usize>) -> Result<String> {
        self.game_mut()?.answer_press(id, &answers)
    }

    /// Mandar o adjunto à conferência.
    pub fn skip_press(&mut self, id: i32) -> Result<String> {
        self.game_mut()?.skip_press(id)
    }

    /// O que confirmar (ou o que falta) na equipa antes de entrar no dia de jogo.
    pub fn lineup_check(&self) -> Result<LineupCheck> {
        self.game()?.lineup_check()
    }

    /// "Continuar Jogo": avança até o próximo evento (jogo do treinador, notícia...).
    pub fn continue_game(&mut self) -> Result<GameInfo> {
        let game = self.game_mut()?;
        game.advance_to_next_event()?;
        game.info()
    }

    /// O jogo do treinador a decorrer (tela da partida).
    pub fn live_match(&self) -> Result<LiveView> {
        self.game()?.live_view()
    }

    /// Joga o jogo do treinador até ao minuto dado.
    pub fn live_step(&mut self, minute: i32) -> Result<LiveView> {
        self.game_mut()?.live_step(minute)
    }

    pub fn live_substitute(&mut self, out: i32, inn: i32) -> Result<LiveView> {
        self.game_mut()?.live_substitute(out, inn)
    }

    pub fn live_swap(&mut self, a: i32, b: i32) -> Result<LiveView> {
        self.game_mut()?.live_swap(a, b)
    }

    pub fn live_tactics(&self, side: i32) -> Result<ClubTactics> {
        self.game()?.live_tactics(side)
    }

    pub fn live_preview_tactics(&self, selection: &Selection) -> Result<ClubTactics> {
        self.game()?.live_preview_tactics(selection)
    }

    pub fn live_set_tactics(&mut self, selection: Selection) -> Result<LiveView> {
        self.game_mut()?.live_set_tactics(&selection)
    }

    pub fn live_instructions(&mut self, instructions: Instructions) -> Result<LiveView> {
        self.game_mut()?.live_instructions(instructions)
    }

    /// Acaba o jogo do treinador e fecha o dia (resultados, notícias).
    pub fn live_finish(&mut self) -> Result<GameInfo> {
        let game = self.game_mut()?;
        game.live_finish()?;
        game.info()
    }

    /// Classificação real de uma competição viva; vazia sem jogo ou numa competição parada.
    pub fn league_table(&self, id: i32) -> Result<Vec<StandingRow>> {
        Ok(self.game.as_ref().map(|g| g.table(id)).unwrap_or_default())
    }

    /// Fases de uma competição viva na época em curso (classificações, grupos, eliminatórias).
    pub fn competition_stages(&self, id: i32) -> Result<Vec<StageView>> {
        Ok(self.game.as_ref().map(|g| g.competition_stages(id)).unwrap_or_default())
    }

    /// Finanças de um clube no jogo em curso.
    pub fn club_finances(&self, id: i32) -> Result<ClubFinances> {
        self.game()?.club_finances(id)
    }

    /// A tela "Números Plantel" do clube do treinador.
    pub fn squad_numbers(&self) -> Result<SquadNumbers> {
        self.game()?.squad_numbers()
    }

    /// O botão Auto: números para quem não tem.
    pub fn auto_squad_numbers(&self, numbers: Vec<NumberAssign>) -> Result<Vec<NumberAssign>> {
        self.game()?.auto_squad_numbers(&numbers)
    }

    /// Grava os números escolhidos pelo treinador.
    pub fn set_squad_numbers(&mut self, numbers: Vec<NumberAssign>) -> Result<SquadNumbers> {
        self.game_mut()?.set_squad_numbers(&numbers)
    }

    /// A direção do clube do treinador: expectativa, confiança e orçamentos.
    pub fn board_view(&self) -> Result<BoardView> {
        self.game()?.board_view()
    }

    pub fn board_confidence(&self) -> Result<BoardConfidence> {
        self.game()?.board_confidence()
    }

    pub fn board_requests(&self) -> Result<Vec<BoardRequest>> {
        self.game()?.board_requests()
    }

    pub fn board_request(&mut self, key: String) -> Result<String> {
        self.game_mut()?.board_request(&key)
    }

    pub fn discipline_options(&self, id: i32) -> Result<DisciplineOptions> {
        self.game()?.discipline_options(id)
    }

    pub fn discipline(&mut self, id: i32, action: usize, reason: String) -> Result<String> {
        self.game_mut()?.discipline(id, action, &reason)
    }

    pub fn jobs_view(&self, nation: Option<i32>) -> Result<JobsView> {
        self.game()?.jobs_view(nation)
    }

    pub fn apply_job(&mut self, id: i32) -> Result<String> {
        self.game_mut()?.apply_job(id)
    }

    pub fn set_reserve(&mut self, id: i32, reserve: bool) -> Result<String> {
        self.game_mut()?.set_reserve(id, reserve)
    }

    pub fn press_comment(&mut self, id: i32, kind: String) -> Result<String> {
        self.game_mut()?.press_comment(id, &kind)
    }

    pub fn league_requests(&self) -> Result<Vec<BoardRequest>> {
        self.game()?.league_requests()
    }

    pub fn league_request(&mut self, key: String) -> Result<String> {
        self.game_mut()?.league_request(&key)
    }

    /// Aba Transferência do perfil de um jogador.
    pub fn transfer_view(&self, id: i32) -> Result<TransferView> {
        self.game()?.transfer_view(id)
    }

    /// Proposta de transferência por um jogador de outro clube (ou por um livre).
    pub fn make_offer(&mut self, id: i32, terms: DealTerms) -> Result<TransferView> {
        // Quem recebe uma proposta, um pedido ou um olheiro entra na Lista Preferencial
        let g = self.game_mut()?;
        let r = g.make_offer(id, &terms)?;
        g.set_shortlist(id, true)?;
        Ok(r)
    }

    /// Pedido de empréstimo de um jogador de outro clube.
    pub fn ask_loan(&mut self, id: i32, terms: LoanTerms) -> Result<TransferView> {
        let g = self.game_mut()?;
        let r = g.ask_loan(id, &terms)?;
        g.set_shortlist(id, true)?;
        Ok(r)
    }

    /// "Pedir Mais" com condições numa proposta recebida.
    pub fn ask_terms(&mut self, id: i32, terms: DealTerms) -> Result<OfferView> {
        self.game_mut()?.ask_terms(id, &terms)
    }

    /// Exercer a opção de compra de um emprestado.
    pub fn exercise_option(&mut self, id: i32) -> Result<TransferView> {
        self.game_mut()?.exercise_option(id)
    }

    /// Funções no clube e o que o jogador pede pela função escolhida.
    pub fn contract_terms(&self, id: i32, role: Option<String>) -> Result<ContractTerms> {
        self.game()?.contract_terms(id, role.as_deref())
    }

    /// Proposta de contrato (renovação ou contratação).
    pub fn negotiate(&mut self, id: i32, role: String, wage: i64, bonus: i64, years: i32) -> Result<ContractReply> {
        let g = self.game_mut()?;
        let r = g.negotiate(id, &role, wage, bonus, years)?;
        g.set_shortlist(id, true)?;
        Ok(r)
    }

    /// Listas de transferências e de empréstimos do plantel.
    pub fn set_listing(&mut self, id: i32, transfer: bool, loan: bool, asking: Option<i64>) -> Result<TransferView> {
        self.game_mut()?.set_listing(id, transfer, loan, asking)
    }

    /// Uma proposta com o texto e os botões.
    pub fn offer_view(&self, id: i32) -> Result<OfferView> {
        self.game()?.offer_view(id)
    }

    /// Resposta do treinador numa proposta.
    pub fn scout_player(&mut self, id: i32) -> Result<String> {
        let g = self.game_mut()?;
        let r = g.scout_player(id)?;
        g.set_shortlist(id, true)?;
        Ok(r)
    }

    pub fn report_staff(&self, kind: String) -> Result<Vec<ReportStaff>> {
        self.game()?.report_staff(&kind)
    }

    pub fn staff_report(&mut self, id: i32, kind: String, staff: Vec<i32>) -> Result<usize> {
        self.game_mut()?.staff_report(id, &kind, &staff)
    }

    pub fn scout_list(&self, slot: i32) -> Result<ScoutList> {
        self.game()?.scout_list(slot)
    }

    pub fn assign_scout(&mut self, slot: i32, kind: String, target: Option<i32>) -> Result<ScoutList> {
        self.game_mut()?.assign_scout(slot, &kind, target)
    }

    pub fn clear_scout(&mut self, slot: i32) -> Result<ScoutList> {
        self.game_mut()?.clear_scout(slot)
    }

    /// Clubes pelo nome (para escolher o clube de um olheiro).
    pub fn search_clubs(&self, text: String) -> Result<Vec<ClubListItem>> {
        self.db().search_clubs(&text)
    }

    pub fn answer_offer(&mut self, id: i32, key: String, fee: Option<i64>) -> Result<OfferView> {
        self.game_mut()?.answer_offer(id, &key, fee)
    }

    /// Entradas e saídas de um clube nos últimos 12 meses.
    pub fn club_transfers(&self, id: i32) -> Result<Vec<TransferItem>> {
        Ok(self.game.as_ref().map(|g| g.club_transfers(id)).transpose()?.unwrap_or_default())
    }

    /// Calendário e resultados de uma competição viva.
    pub fn list_fixtures(&self, id: i32) -> Result<Vec<Fixture>> {
        Ok(self.game.as_ref().map(|g| g.fixtures(id)).unwrap_or_default())
    }

    /// Ficha do "Próximo Jogo".
    pub fn match_preview(&self, id: i32) -> Result<MatchPreview> {
        self.game()?.match_preview(id)
    }

    /// Partidas de um clube, por data.
    pub fn club_fixtures(&self, id: i32) -> Result<Vec<Fixture>> {
        Ok(self.game.as_ref().map(|g| g.club_fixtures(id)).unwrap_or_default())
    }

    pub fn set_shortlist(&mut self, id: i32, on: bool) -> Result<()> {
        self.game_mut()?.set_shortlist(id, on)
    }

    pub fn news_links(&self, id: i32) -> Result<Vec<NewsLink>> {
        self.game()?.news_links(id)
    }

    pub fn list_news(&self) -> Result<Vec<NewsItem>> {
        let game = self.game()?;
        let mut news = game.news().to_vec();
        // As mensagens cuja proposta ainda espera uma resposta do treinador
        for n in &mut news {
            if let Some(o) = n.offer_id {
                n.awaiting = game.offer_view(o).is_ok_and(|v| v.actions.iter().any(|a| a.key != "withdraw"));
            }
            // As conferências de imprensa por responder
            if let Some(id) = n.link.as_deref().and_then(|l| l.strip_prefix("press:")).and_then(|i| i.parse().ok()) {
                n.awaiting = game.press_view(id).is_ok_and(|v| v.status == "pending");
            }
        }
        Ok(news)
    }

    pub fn read_news(&mut self, id: i32) -> Result<GameInfo> {
        let game = self.game_mut()?;
        game.mark_news_read(id);
        game.info()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_as_copies_and_continues_in_the_new_file() {
        let Some(path) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-gravar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Session::open(&path, &dir).unwrap();
        let new = NewGame {
            save_name: "Original".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
        };
        s.new_game(&new).unwrap();
        s.save_as("Cópia".into(), false).unwrap();
        let saves = s.list_saves().unwrap();
        assert!(saves.iter().any(|x| x.name == "Cópia") && saves.iter().any(|x| x.name == "Original"));
        // O mesmo nome outra vez: pergunta antes de substituir
        s.save_as("Original".into(), false).unwrap_err();
        s.save_as("Original".into(), true).unwrap();
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

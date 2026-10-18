//! O que o app tem aberto: o mundo base (enciclopédia) e, se houver, um jogo em curso.
//! Usado igual pelo Tauri e pela ponte de dev, para os dois nunca divergirem.

use std::path::{Path, PathBuf};

use crate::{
    ClubProfile, ClubTactics, Error, Fixture, Game, GameInfo, NewGame, NewsItem, PersonProfile, Result, SaveSummary, Selection,
    StandingRow, World, list_saves,
};

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

    /// Perfil com o estado do jogador no jogo em curso (condição, moral, lesão...).
    pub fn get_person(&self, id: i32) -> Result<PersonProfile> {
        let mut p = self.db().get_person(id)?;
        p.status = self.game.as_ref().and_then(|g| g.player_status(id));
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

    /// Grava a escolha do treinador.
    pub fn set_tactics(&mut self, selection: Selection) -> Result<ClubTactics> {
        self.game_mut()?.set_selection(selection)
    }

    pub fn game_info(&self) -> Result<Option<GameInfo>> {
        self.game.as_ref().map(Game::info).transpose()
    }

    pub fn list_saves(&self) -> Result<Vec<SaveSummary>> {
        list_saves(&self.saves_dir)
    }

    pub fn new_game(&mut self, new: &NewGame) -> Result<GameInfo> {
        // A seed só precisa variar entre jogos; a partir dela tudo é reproduzível
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as i64 & i64::MAX);
        let game = Game::create(&self.world_path, &self.saves_dir, new, seed)?;
        let info = game.info()?;
        self.game = Some(game);
        Ok(info)
    }

    /// Abre um save da pasta de saves pelo nome do arquivo (nunca um caminho qualquer).
    pub fn load_game(&mut self, file: &str) -> Result<GameInfo> {
        let name = Path::new(file).file_name().ok_or_else(|| Error::Invalid("Arquivo inválido.".into()))?;
        let game = Game::load(&self.saves_dir.join(name))?;
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

    /// "Continuar Jogo": avança até o próximo evento (jogo do treinador, notícia...).
    pub fn continue_game(&mut self) -> Result<GameInfo> {
        let game = self.game_mut()?;
        game.advance_to_next_event()?;
        game.info()
    }

    /// Classificação real de uma competição viva; vazia sem jogo ou numa competição parada.
    pub fn league_table(&self, id: i32) -> Result<Vec<StandingRow>> {
        Ok(self.game.as_ref().map(|g| g.table(id)).unwrap_or_default())
    }

    /// Calendário e resultados de uma competição viva.
    pub fn list_fixtures(&self, id: i32) -> Result<Vec<Fixture>> {
        Ok(self.game.as_ref().map(|g| g.fixtures(id)).unwrap_or_default())
    }

    /// Partidas de um clube, por data.
    pub fn club_fixtures(&self, id: i32) -> Result<Vec<Fixture>> {
        Ok(self.game.as_ref().map(|g| g.club_fixtures(id)).unwrap_or_default())
    }

    pub fn list_news(&self) -> Result<Vec<NewsItem>> {
        Ok(self.game()?.news().to_vec())
    }

    pub fn read_news(&mut self, id: i32) -> Result<GameInfo> {
        let game = self.game_mut()?;
        game.mark_news_read(id);
        game.info()
    }
}

//! O que o app tem aberto: o mundo base (enciclopédia) e, se houver, um jogo em curso.
//! Usado igual pelo Tauri e pela ponte de dev, para os dois nunca divergirem.

use std::path::{Path, PathBuf};

use crate::{Error, Game, GameInfo, NewGame, NewsItem, Result, SaveSummary, World, list_saves};

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

    pub fn list_news(&self) -> Result<Vec<NewsItem>> {
        Ok(self.game()?.news().to_vec())
    }

    pub fn read_news(&mut self, id: i32) -> Result<GameInfo> {
        let game = self.game_mut()?;
        game.mark_news_read(id);
        game.info()
    }
}

import ManagerDialogs from "./ManagerDialogs";
import type { ManagerDialog } from "./managerMenu";
import type { LineupCheck } from "../bindings/LineupCheck";
import LineupDialog from "./LineupDialog";
import { useState } from "react";
import { hasBackend } from "../api";
import Loading from "../components/Loading";
import MatchDay from "../components/MatchDay";
import OptionsDialog from "../components/OptionsDialog";
import { useGame } from "../game";
import { useNav } from "../nav";
import LoadGamePage from "../pages/LoadGamePage";
import NewGamePage from "../pages/NewGamePage";
import StartScreen from "../pages/StartScreen";
import ExitDialog from "./ExitDialog";
import Notice from "./Notice";
import SackedDialog from "./SackedDialog";
import Screens from "./Screens";
import SideBar from "./SideBar";
import TopBar from "./TopBar";
import { useContinueGame } from "./useContinueGame";
import { useMatchDay } from "./useMatchDay";
import { useNotice } from "./useNotice";

/** A janela do jogo: a coluna à esquerda e, ao lado, a tela do modo atual e o que vem por cima. */
export default function Shell() {
  const game = useGame();
  const nav = useNav();
  const [options, setOptions] = useState(false);
  const [exiting, setExiting] = useState(false);
  const [processing, setProcessing] = useState(false);
  // O aviso do despedimento, até o treinador ir ler as notícias
  const [sackedSeen, setSackedSeen] = useState(false);
  const { notice, notify } = useNotice();
  const matchDay = useMatchDay();
  // O aviso ou a confirmação da equipa antes do dia de jogo
  const [lineup, setLineup] = useState<{ check: LineupCheck; go: () => void } | null>(null);
  const [dialog, setDialog] = useState<ManagerDialog | null>(null);
  const browsing = game.mode === "game" || game.mode === "explore";
  const { advance, busy } = useContinueGame({
    notify,
    onProcessing: setProcessing,
    onMatch: matchDay.open,
    matchStep: matchDay.advance,
    onLineup: (check, go) => setLineup({ check, go }),
  });

  return (
    <div className="flex h-full flex-col">
      <TopBar onContinue={() => void advance()} busy={busy} />
      <div className="flex min-h-0 flex-1">
        <SideBar
          onOptions={() => setOptions((o) => !o)}
          onExit={() => setExiting(true)}
          onDialog={setDialog}
        />
        <main className="cm-backdrop relative min-w-0 flex-1">
          {!hasBackend && (
            <div className="p-6 text-[#aab3c4]">
              Esta tela lê o banco pelo Rust. Abra pelo app desktop:{" "}
              <code className="text-cm-yellow">npm run tauri dev</code>
            </div>
          )}
          {hasBackend && browsing && <Screens />}
          {game.mode === "start" && (
            <StartScreen onOptions={() => setOptions(true)} />
          )}
          {game.mode === "newGame" && <NewGamePage />}
          {game.mode === "loadGame" && <LoadGamePage />}
          {game.mode === "game" &&
            game.info?.sacked &&
            !matchDay.match &&
            !sackedSeen && (
              <SackedDialog
                onRead={() => {
                  setSackedSeen(true);
                  nav.go({ kind: "news" });
                }}
                onJobs={() => {
                  setSackedSeen(true);
                  nav.go({ kind: "jobs" });
                }}
              />
            )}
          {exiting && (
            <ExitDialog onCancel={() => setExiting(false)} notify={notify} />
          )}
          {options && <OptionsDialog onClose={() => setOptions(false)} />}
          {processing && (
            <Loading title="Continuar Jogo" detail="Processando, Por Favor Espere…" delay={250} />
          )}
          {/* Escondido fica montado: ao voltar continua na mesma fase (o jogo pode já ter acabado no core) */}
          {matchDay.match && game.info && (
            <div className={matchDay.hidden ? "hidden" : "contents"}>
              <MatchDay
                fixture={matchDay.match.fixture}
                dayGames={matchDay.match.dayGames}
                table={matchDay.match.table}
                own={game.info.club.id}
                step={matchDay.step}
                active={!matchDay.hidden}
                onDone={matchDay.close}
              />
            </div>
          )}
          {lineup && game.info && (
            <LineupDialog
              check={lineup.check}
              onTactics={() => {
                setLineup(null);
                nav.go({ kind: "tactics", id: game.info!.club.id });
              }}
              onCancel={() => setLineup(null)}
              onGo={() => {
                const go = lineup.go;
                setLineup(null);
                go();
              }}
            />
          )}
          {dialog && game.info && <ManagerDialogs dialog={dialog} onClose={() => setDialog(null)} notify={notify} />}
          {notice && <Notice text={notice} />}
        </main>
      </div>
    </div>
  );
}

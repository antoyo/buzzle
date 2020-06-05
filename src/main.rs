/*
 * Use the database at:
 * https://www.bughouse-db.org/
 * and chess engine sjeng (is there a better one? maybe Sunsetter?)
 * to create bughouse puzzles.
 *
 * Start with simple tactics like forced checkmate or forced piece capture.
 * Make the engine thinks that the opponent has all the pieces in hand to account for the fact that
 * bughouse is time-based and his pieces can change at any time.
 *
 * Show the partern's board and have some puzzle where the solution is to win a piece so that the
 * partner can do a forced checkmate.
 *
 * Also use the games from chess.com.
 * They're all accessible on this link: https://www.chess.com/live/game/<SEQUENTIAL_GAME_ID>
 * In the info tab, we can even see the partner's game.
 *
 * Find puzzles where you can force capture piece (i.e. checked fork, pawn on b7 with unmovable
 * rook on a8, …).
 *
 * Add buttons to ask for the piece you need (request only before first move?).
 *
 * Add button for sit (the solution would be sit when any move will make your partner lose).
 *
 * Have puzzles where the solution is to survive the longest (when most leads to checkmate).
 *
 * Use BFEN in the PGN: https://bughousedb.com/Lieven_BPGN_Standard.txt
 *
 * If losing a piece before check-mating would make your partner's opponent win, don't consider it
 * as a solution.
 */

extern crate chessground;
extern crate encoding_rs;
extern crate gdk;
extern crate gtk;
extern crate pgn_reader;
#[macro_use]
extern crate relm;
#[macro_use]
extern crate relm_derive;
extern crate shakmaty;

use std::cmp::min;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use chessground::{
    DrawBrush,
    DrawShape,
    Ground,
    GroundMsg::{self, SetBoard, SetPockets, SetPos, UserDrop, UserMove},
    Pos,
};
use gtk::{
    ButtonExt,
    ButtonsType,
    DialogExt,
    DialogFlags,
    FileChooserAction,
    FileChooserDialog,
    FileChooserExt,
    Inhibit,
    LabelExt,
    MessageDialog,
    MessageType,
    OrientableExt,
    Orientation::Vertical,
    ResponseType,
    ToolButtonExt,
    WidgetExt,
};
use pgn_reader::{
    BufferedReader,
    RawHeader,
    SanPlus,
    Visitor,
};
use relm::{Relm, Widget, timeout};
use relm_derive::widget;
use shakmaty::{
    Board,
    Chess,
    fen::Fen,
    FromSetup,
    Material,
    Move,
    Piece,
    Position,
    position::Bughouse,
    Role,
    Setup,
    Square,
};

use self::Msg::*;

#[derive(Msg)]
pub enum Msg {
    Flip,
    ImportPGN,
    MovePlayed(Square, Square, Option<Role>),
    NextPuzzle,
    PieceDrop(Piece, Square),
    PlayOpponentMove,
    PreviousPuzzle,
    Quit,
}

#[derive(Clone)]
struct TrainingPosition {
    annotations: Vec<Shape>,
    position: Board,
}

pub struct Model {
    can_play: bool,
    current_move: usize,
    current_position: Bughouse,
    current_puzzle: usize,
    puzzles: Vec<Puzzle>,
    relm: Relm<Win>,
    text: &'static str,
}

#[derive(Clone)]
struct Shape {
    orig: Square,
    dest: Square,
    brush: DrawBrush,
}

impl PartialEq<DrawShape> for Shape {
    fn eq(&self, rhs: &DrawShape) -> bool {
        self.orig == rhs.orig() && self.dest == rhs.dest() && self.brush == rhs.brush()
    }
}

#[widget]
impl Widget for Win {
    fn model(relm: &Relm<Self>, _: ()) -> Model {
        Model {
            can_play: true,
            current_move: 0,
            current_position: Bughouse::default(),
            current_puzzle: 0,
            puzzles: vec![],
            relm: relm.clone(),
            text: "",
        }
    }

    fn update(&mut self, event: Msg) {
        match event {
            Flip => self.ground.emit(GroundMsg::Flip),
            ImportPGN => {
                let dialog = FileChooserDialog::with_buttons(
                    Some("Select a PGN file to import"),
                    Some(&self.window),
                    FileChooserAction::Open,
                    &[("Import", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
                );
                let dir = env::current_dir().expect("current dir").join("tests");
                dialog.set_current_folder(dir);
                if dialog.run() == ResponseType::Ok {
                    for filename in dialog.get_filenames() {
                        if let Err(error) = self.import_file(&filename) {
                            let message_dialog = MessageDialog::new(Some(&self.window), DialogFlags::empty(), MessageType::Error, ButtonsType::Ok, &error);
                            message_dialog.run();
                            message_dialog.destroy();
                        }
                    }
                }
                dialog.destroy();
            },
            MovePlayed(orig, dest, promotion) => {
                if !self.model.can_play {
                    return;
                }

                self.model.text = "";
                let legals = self.model.current_position.legals();
                let mov = legals.iter().find(|mov| {
                    mov.from() == Some(orig) && mov.to() == dest &&
                    mov.promotion() == promotion
                });

                self.try_move(mov);
            },
            NextPuzzle => {
                self.model.current_move = 0;
                self.model.current_puzzle = min(self.model.current_puzzle + 1, self.model.puzzles.len() - 1);
                self.show_position();
            },
            PieceDrop(piece, to) => {
                if !self.model.can_play {
                    return;
                }

                let legals = self.model.current_position.legals();
                let mov = Move::Put {
                    role: piece.role,
                    to,
                };
                if legals.contains(&mov) {
                    self.try_move(Some(&mov));
                }
            },
            PlayOpponentMove => {
                if let Some(puzzle) = self.model.puzzles.get(self.model.current_puzzle) {
                    if let Some(current_move) = puzzle.moves.get(self.model.current_move) {
                        self.model.can_play = true;
                        self.model.current_move += 1;
                        self.model.current_position.play_unchecked(current_move);
                        self.ground.emit(SetPos(Pos::new(&self.model.current_position)));
                    }
                }
            },
            PreviousPuzzle => {
                self.model.current_move = 0;
                if self.model.current_puzzle > 0 {
                    self.model.current_puzzle -= 1;
                }
                self.show_position();
            },
            Quit => gtk::main_quit(),
        }
    }

    fn import_file(&mut self, filename: &PathBuf) -> Result<(), String> {
        let mut file = File::open(filename).map_err(|error| error.to_string())?;
        let mut data = vec![];
        file.read_to_end(&mut data).map_err(|error| error.to_string())?;
        let (result, _, _) = encoding_rs::WINDOWS_1252.decode(&data);

        let mut importer = FENImporter::new();
        let mut reader = BufferedReader::new_cursor(result.as_bytes());
        reader.read_all(&mut importer).map_err(|_| "Cannot parse PGN file")?;
        self.model.puzzles = importer.puzzles;
        self.model.current_puzzle = 0;
        self.model.current_move = 0;
        self.model.can_play = true;
        self.model.text = "";
        self.show_position();
        Ok(())
    }

    fn show_position(&mut self) {
        if let Some(puzzle) = self.model.puzzles.get(self.model.current_puzzle) {
            self.model.current_position = puzzle.position.clone();
            let pos = Pos::new(&puzzle.position);
            let turn = puzzle.position.turn();
            self.ground.emit(SetPos(pos));
            self.ground.emit(SetPockets(puzzle.position.pockets().cloned().unwrap_or(Material::new()), turn));
        }
    }

    fn try_move(&mut self, mov: Option<&Move>) {
        if let Some(puzzle) = self.model.puzzles.get(self.model.current_puzzle) {
            if let Some(current_move) = puzzle.moves.get(self.model.current_move) {
                if let Some(mov) = mov {
                    if mov == current_move {
                        self.model.current_move += 1;
                        let turn = self.model.current_position.turn();
                        self.model.current_position.play_unchecked(mov);
                        self.ground.emit(SetPos(Pos::new(&self.model.current_position)));
                        self.ground.emit(SetPockets(self.model.current_position.pockets().cloned().unwrap_or(Material::new()), turn));
                        self.model.can_play = false;

                        if self.model.current_move == puzzle.moves.len() {
                            self.model.text = "Success";
                        }
                        else {
                            timeout(self.model.relm.stream(), 1000, || PlayOpponentMove);
                        }
                    }
                    else {
                        self.model.text = "Wrong answer";
                    }
                }
            }
        }
    }

    view! {
        #[name="window"]
        gtk::Window {
            gtk::Box {
                orientation: Vertical,
                gtk::Toolbar {
                    gtk::ToolButton {
                        icon_name: Some("document-open"),
                        label: Some("Import PGN files"),
                        clicked => ImportPGN,
                    },
                    gtk::ToolButton {
                        icon_name: Some("object-flip-vertical"),
                        label: Some("Flip board"),
                        clicked => Flip,
                    },
                    gtk::ToolButton {
                        icon_name: Some("application-exit"),
                        label: Some("Quit"),
                        clicked => Quit,
                    },
                },
                #[name="ground"]
                Ground {
                    UserMove(orig, dest, promotion) => MovePlayed(orig, dest, promotion),
                    UserDrop(piece, to) => PieceDrop(piece, to),
                },
                gtk::ButtonBox {
                    gtk::Button {
                        label: "Précédent",
                        clicked => PreviousPuzzle,
                    },
                    gtk::Button {
                        label: "Suivant",
                        clicked => NextPuzzle,
                    },
                },
                #[name="label"]
                gtk::Label {
                    text: &self.model.text,
                },
            },
            delete_event(_, _) => (Quit, Inhibit(false)),
        }
    }
}

struct Puzzle {
    moves: Vec<Move>,
    position: Bughouse,
}

struct FENImporter {
    current_position: Bughouse,
    puzzles: Vec<Puzzle>,
}

impl FENImporter {
    fn new() -> Self {
        Self {
            current_position: Bughouse::default(),
            puzzles: vec![],
        }
    }
}

impl Visitor for FENImporter {
    type Result = ();

    fn begin_game(&mut self) {
    }

    fn end_game(&mut self) -> Self::Result {
    }

    fn header(&mut self, key: &[u8], value: RawHeader) {
        if key == b"FEN" {
            let fen = value.as_bytes();
            match fen.iter().position(|&byte| byte == b'|') {
                Some(index) => {
                    let player = &fen[..index - 1];
                    let partner = &fen[index + 1..];
                    match Fen::from_ascii(player) {
                        Ok(fen) => {
                            match Bughouse::from_setup(&fen) {
                                Ok(setup) => {
                                    self.current_position = setup.clone();
                                    self.puzzles.push(Puzzle {
                                        moves: vec![],
                                        position: setup,
                                    });
                                },
                                Err(error) => {
                                    eprintln!("Error setup position: {}", error);
                                },
                            }
                        },
                        Err(error) => {
                            eprintln!("Error parsing FEN: {}", error);
                        },
                    }
                },
                None => {
                    eprintln!("Cannot find | in FEN.");
                },
            }
        }
    }

    fn san(&mut self, san_plus: SanPlus) {
        if let Some(puzzle) = self.puzzles.last_mut() {
            match san_plus.san.to_move(&self.current_position) {
                Ok(mov) => {
                    self.current_position.play_unchecked(&mov);
                    puzzle.moves.push(mov);
                },
                Err(error) => eprintln!("Error playing move: {:?}", error),
            }
        }
    }
}

fn main() {
    Win::run(()).expect("window run");
}

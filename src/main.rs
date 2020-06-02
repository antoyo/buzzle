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
 */

fn main() {
    println!("Hello, world!");
}

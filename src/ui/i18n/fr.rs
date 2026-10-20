//! Тексты интерфейса на французском.

use super::{Plural, PluralForm, Texts};

/// В отличие от английского, единственное число и у нуля: «0 piste», «1 piste», «2 pistes»
fn plural_form(count: usize) -> PluralForm {
    if count <= 1 {
        PluralForm::One
    } else {
        PluralForm::Many
    }
}

pub static TEXTS: Texts = Texts {
    plural_form,

    library: "Bibliothèque",
    now_playing: "Lecture en cours",
    artists: "Artistes",
    albums: "Albums",
    songs: "Morceaux",
    playlist: "Liste de lecture",
    settings: "Réglages",

    track_count: Plural::two("piste", "pistes"),
    song_count: Plural::two("morceau", "morceaux"),
    album_count: Plural::two("album", "albums"),
    artist_count: Plural::two("artiste", "artistes"),
    more_file_count: Plural::two("autre fichier", "autres fichiers"),

    no_tracks: "Aucune piste",
    library_empty: "La bibliothèque est vide",
    no_results: "Aucun résultat",
    no_albums: "Aucun album",

    search: "Rechercher",
    found_of: "{found} sur {tracks}",

    open_files: "Ouvrir des fichiers",
    add_files: "Ajouter des fichiers",
    add_folder: "Ajouter un dossier",
    audio_files: "Audio",
    adding_progress: "Ajout : {done} sur {total}",
    updating_library_progress: "Mise à jour de la bibliothèque : {done} sur {total}",
    scanning_folder: "Analyse du dossier…",
    no_audio_found: "Aucun fichier audio pris en charge trouvé",
    more_files_failed: "…impossible d'ajouter {more}",

    show_artist: "Afficher l'artiste",
    show_album: "Afficher l'album",
    show_current_track: "Afficher la piste en cours",
    file_missing: "Fichier introuvable : il a été déplacé ou supprimé",
    shuffle: "Lecture aléatoire",
    repeat_all: "Tout répéter",
    repeat_one: "Répéter un morceau",

    play: "Lire",
    play_all: "Tout lire",
    add_to_playlist: "Ajouter à la liste de lecture",
    remove_from_library: "Supprimer de la bibliothèque",
    remove_from_playlist: "Retirer de la liste de lecture",
    play_items: "Lire {items}",
    add_items_to_playlist: "Ajouter {items} à la liste de lecture",
    remove_items_from_library: "Supprimer {items} de la bibliothèque",
    remove_items_from_playlist: "Retirer {items} de la liste de lecture",
    remove: "Supprimer",
    cancel: "Annuler",
    ok: "OK",
    done: "Terminé",

    language: "Langue",

    remove_album_title: "Supprimer l'album de la bibliothèque ?",
    remove_album_message: "« {album} » sera supprimé de votre bibliothèque. \
                           Les fichiers resteront sur votre disque.",
    remove_albums_title: "Supprimer {albums} de la bibliothèque ?",
    remove_albums_message: "{albums} ({tracks}) seront supprimés de votre bibliothèque. \
                            Les fichiers resteront sur votre disque.",
    remove_artist_title: "Supprimer l'artiste de la bibliothèque ?",
    remove_artist_message: "Toutes les pistes de « {artist} » seront supprimées de votre \
                            bibliothèque. Les fichiers resteront sur votre disque.",
    remove_artists_title: "Supprimer {artists} de la bibliothèque ?",
    remove_artists_message: "Toutes les pistes de {artists} ({tracks}) seront supprimées de \
                             votre bibliothèque. Les fichiers resteront sur votre disque.",
    remove_song_title: "Supprimer le morceau de la bibliothèque ?",
    remove_song_message: "« {song} » sera supprimé de votre bibliothèque. \
                          Le fichier restera sur votre disque.",
    remove_songs_title: "Supprimer {songs} de la bibliothèque ?",
    remove_songs_message: "{songs} seront supprimés de votre bibliothèque. \
                           Les fichiers resteront sur votre disque.",
};

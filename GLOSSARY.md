# Dirpick

Dirpick combines filesystem navigation, search, filtering, and previews in a
terminal. These terms describe its search interaction.

## Language

**Search mode**:
The kind of candidate sought by a search: Files, Directories, or Contents.

**Files mode**:
The search mode for finding file paths.

**Directories mode**:
The search mode for finding directory paths.

**Contents mode**:
The search mode for finding text within files.

**Search scope**:
The directory beneath which a search considers candidates.
_Avoid_: Serving root

**Project root**:
The top-level directory of the Git working tree containing the launch CWD.

**Launch CWD**:
The working directory from which Dirpick was launched, independent of later
browsing within Dirpick.

**Query**:
The text used to match candidates in the selected search mode.

**Preview**:
The inspection view associated with the selected search result.

**Include filter**:
A constraint on eligible paths, evaluated relative to the search scope in
every search mode.

**Exclude filter**:
A constraint that removes paths from consideration even when the include
filter matches them.

**Browse**:
Moving into a directory as the active search scope while remaining in Dirpick.
_Avoid_: cd

**cd & exit**:
The action that selects a directory for the invoking shell and exits Dirpick.

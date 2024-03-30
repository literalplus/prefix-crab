library(readr)
library(ggplot2)
library(gridExtra)
library(egg)
library(ggpubr)
library(dplyr)


at10 <- read_csv("at10_all.csv")
at11 <- read_csv("at11_all.csv")
u3 <- read_csv("u3_all.csv")

###

analyse_changes <- function(label, base) {
  once_or_never <- base |> filter(run_count <= 2)
  twice_or_more_cnt = nrow(base) - nrow(once_or_never)
  twice_or_more_pct = twice_or_more_cnt / nrow(base) * 100
  once_cnt = nrow(base |> filter(run_count == 2))
  once_pct = once_cnt / nrow(base) * 100
  never_cnt = nrow(base |> filter(run_count == 1))
  never_pct = never_cnt / nrow(base) * 100
  cat(paste0(
    paste("\n---", label, "--- \n"),
    paste0("nodes total: ", nrow(base), "\n"),
    paste0(" - no change: ", never_cnt, " -% ", never_pct, "\n"),
    paste0(" - one change: ", once_cnt, " -% ", once_pct, "\n"),
    paste0(" - more changes: ", twice_or_more_cnt, " -% ", twice_or_more_pct, "\n"),
    paste0("max changes: ", max(base$run_count) - 1, "\n")
  ))
}

# chart was attempted but the magnitudes are so different that it makes no sense

percent_of <- function(mine, base) {
  paste0(formatC(100 * ((nrow(mine))/nrow(base)), format = "f", digits = 0), "%")
}

analyse_per_verdict <- function(label, base) {
  all_verdicts <- sort(unique(base$last_run))
  for(verdict in all_verdicts) {
    filtered <- base |> filter(last_run == verdict)
    analyse_changes(
      paste0(label, " / Verdict: ", verdict, " / ", percent_of(filtered, base)),
      filtered
    )
  }
}

sink("analysis_at10.txt")
analyse_changes("AT-10", at10)
analyse_per_verdict("AT-10", at10)
sink()

sink("analysis_at11.txt")
analyse_changes("AT-11", at11)
analyse_per_verdict("AT-11", at11)
sink()

sink("analysis_u3.txt")
analyse_changes("U-3", u3)
analyse_per_verdict("U-3", u3)
sink()

# 
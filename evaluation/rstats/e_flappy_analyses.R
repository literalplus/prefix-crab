library(readr)
library(ggplot2)
library(gridExtra)
library(egg)
library(ggpubr)
library(dplyr)


at10 <- read_csv("at10_3.csv")
at11 <- read_csv("at11_3.csv")
u3 <- read_csv("u3_3.csv")

NET_BREAKS <- c(29, 32, 40, 48, 56, 64)

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
analyse_changes("AT-10 (potential split)", at10 |> filter(is_eligible_for_split) |> filter(confidence < 255))
analyse_changes("AT-10 (confident leaves)", at10 |> filter(is_eligible_for_split) |> filter(confidence == 255))
analyse_changes("AT-10 (internal nodes)", at10 |> filter(!is_eligible_for_split))
analyse_per_verdict("AT-10", at10)
analyse_per_verdict("AT-10 (potential split)", at10 |> filter(is_eligible_for_split) |> filter(confidence < 255))
analyse_per_verdict("AT-10 (internal nodes)", at10 |> filter(!is_eligible_for_split))
sink()

sink("analysis_at11.txt")
analyse_changes("AT-11", at11)
analyse_changes("AT-11 (potential split)", at11 |> filter(is_eligible_for_split) |> filter(confidence < 255))
analyse_changes("AT-11 (confident leaves)", at11 |> filter(is_eligible_for_split) |> filter(confidence == 255))
analyse_changes("AT-11 (cinternal nodes)", at11 |> filter(!is_eligible_for_split))
analyse_per_verdict("AT-11", at11)
analyse_per_verdict("AT-11 (potential split)", at11 |> filter(is_eligible_for_split) |> filter(confidence < 255))
analyse_per_verdict("AT-11 (internal nodes)", at11 |> filter(!is_eligible_for_split))
sink()

sink("analysis_u3.txt")
analyse_changes("U-3", u3)
analyse_changes("U-3 (potential split)", u3 |> filter(is_eligible_for_split) |> filter(confidence < 255))
analyse_changes("U-3 (internal nodes)", u3 |> filter(!is_eligible_for_split))
analyse_per_verdict("U-3", u3)
analyse_per_verdict("U-3 (potential split)", u3 |> filter(is_eligible_for_split) |> filter(confidence < 255))
analyse_per_verdict("U-3 (internal nodes)", u3 |> filter(!is_eligible_for_split))
sink()


### average run length

avg_run_len_hist <- function(base, label) {
  grouped_len <- base |>
    group_by(net_len) |>
    summarise(run_len_avg = mean(run_len_avg))
  ggplot(data = grouped_len, aes(x = net_len, y = run_len_avg)) +
    geom_bar(stat="identity", fill = "#9967bc") +
    labs(title = NULL, x = paste("Prefix Length -", label)) +
    scale_y_continuous(name = "Average Run Length") +
    scale_x_continuous(breaks=NET_BREAKS)
}
avg_analysis_count_hist <- function(base, label) { # avg_run_len * num_runs = total_len
  grouped_len <- base |>
    mutate(analysis_count = run_len_avg * run_count) |>
    group_by(net_len) |>
    summarise(analysis_count = mean(analysis_count))
  ggplot(data = grouped_len, aes(x = net_len, y = analysis_count)) +
    geom_bar(stat="identity", fill = "#9967bc") +
    labs(title = NULL, x = paste("Prefix Length -", label)) +
    scale_y_continuous(name = "Average Analysis Count") +
    scale_x_continuous(breaks=NET_BREAKS)
}

arl_at10 <- avg_run_len_hist(at10, "AT-10")
arl_at11 <- avg_run_len_hist(at11, "AT-11")
arl_u3 <- avg_run_len_hist(u3, "U-3")

arl <- ggarrange(arl_at10, arl_at11, ncol = 2)
ggsave("Eval-Flappy-AvgRunLen.pdf", arl, units="cm", width=19, height=6)


aac_at10 <- avg_analysis_count_hist(at10, "AT-10")
aac_at11 <- avg_analysis_count_hist(at11, "AT-11")
aac_u3 <- avg_analysis_count_hist(u3, "U-3")

aac <- ggarrange(aac_at10, aac_at11, ncol = 2)
ggsave("Eval-Flappy-AvgAnalysisCnt.pdf", aac, units="cm", width=19, height=6)


###

confidence_per_len <- function(base, label) {
  grouped_len <- base |>
    group_by(net_len) |>
    summarise(confidence_median = mean(confidence))
  ggplot(data = grouped_len, aes(x = net_len, y = confidence_median)) +
    geom_bar(stat="identity", fill = "#9967bc") +
    labs(title = NULL, x = paste("Prefix Length -", label)) +
    scale_y_continuous(name = "Median Confidence") +
    scale_x_continuous(breaks=NET_BREAKS)
}

confidence_per_len(at10, "AT-10")
confidence_per_len(at11, "AT-11")
confidence_per_len(u3, "U-3")

###

nonflappy_start_evidence <- function(base, label) {
  grouped_len <- base |>
    filter(run_count == 1) |>
    group_by(net_len) |>
    summarise(confidence_median = median(last_run_start_evidence))
  ggplot(data = grouped_len, aes(x = net_len, y = confidence_median)) +
    geom_bar(stat="identity", fill = "#9967bc") +
    labs(title = NULL, x = paste("Prefix Length -", label)) +
    scale_y_continuous(name = "Median Evidence (No Changes)") +
    scale_x_continuous(breaks=NET_BREAKS)
}


nonflappy_start_evidence(at10, "AT-10")
nonflappy_start_evidence(at11, "AT-11")
nonflappy_start_evidence(u3, "U-3")

###

actionable_leaves_per_len <- function(base, label) {
  grouped_len <- base |>
    filter(is_eligible_for_split) |>
    filter(last_run_should_split) |>
    group_by(net_len) |>
    summarise(leaf_count = n())
  ggplot(data = grouped_len, aes(x = net_len, y = leaf_count)) +
    geom_bar(stat="identity", fill = "#9967bc") +
    labs(title = NULL, x = paste("Prefix Length -", label)) +
    scale_y_continuous(name = "# Leaves") +
    scale_x_continuous(breaks=NET_BREAKS)
}

alpl_at10 <- actionable_leaves_per_len(at10, "AT-10")
alpl_at11 <- actionable_leaves_per_len(at11, "AT-11")
alpl_u3 <- actionable_leaves_per_len(u3, "U-3")


alpl <- ggarrange(alpl_at10, alpl_at11, ncol = 2)
ggsave("Eval-Flappy-ActionablePrefixes.pdf", alpl, units="cm", width=19, height=6)



###


nonleaves_per_len <- function(base, label) {
  grouped_len <- base |>
    filter(!is_eligible_for_split) |>
    group_by(net_len) |>
    summarise(leaf_count = n())
  ggplot(data = grouped_len, aes(x = net_len, y = leaf_count)) +
    geom_bar(stat="identity", fill = "#9967bc") +
    labs(title = NULL, x = paste("Prefix Length -", label)) +
    scale_y_continuous(name = "# Leaves") +
    scale_x_continuous(breaks=NET_BREAKS)
}

nonleaves_per_len(at10, "AT-10")
nonleaves_per_len(at11, "AT-11")
nonleaves_per_len(u3, "U-3")

###
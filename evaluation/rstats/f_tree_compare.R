library(readr)
library(ggplot2)
library(gridExtra)
library(egg)
library(ggpubr)
library(dplyr)
library(stringr)

dat_uni <- read_csv("at_compare_uni.csv")
dat_as <- read_csv("at_compare_2a0e_97c0_ce0__44.csv")
dat_huge <- read_csv("at_compare_all.csv")

NET_BREAKS <- c(29, 32, 40, 48, 56, 64)
PRESENCES_COLORS <- c(
  "Different" = "#9967bc",
  "CandidateDifferent" = "#d19cf5",
  "CandidateSame" = "#a1eae4",
  "EvalOnly" = "#ffd059",
  "ReferenceOnly" = "#d29630",
  "Same" = "#58dcd2" #"#34b0a7"
)
PRESENCES_ORDER = c(
  "Different",
  "CandidateDifferent",
  "CandidateSame",
  "Same",
  "EvalOnly",
  "ReferenceOnly"
)

scale_fill_presences <- function() {
  scale_fill_manual(values=PRESENCES_COLORS, breaks=PRESENCES_ORDER, name=NULL)
}

### - AS presences per prefix len

make_presences_rel <- function(with_legend) {
  ggplot(data = dat_as, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
    geom_bar(position = "fill", show.legend = with_legend) +
    scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
    scale_y_continuous(name = "% of Nodes", labels = scales::percent) +
    theme(legend.direction='horizontal', legend.box='horizontal') +
    theme(legend.text=element_text(size=10)) +
    guides(fill = guide_legend(nrow = 1)) +
    scale_fill_presences()
}

as_presences_abs <- ggplot(data = dat_as, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(show.legend = FALSE) +
  scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()

the_legend <- as_ggplot(get_legend(make_presences_rel(TRUE)))

ggsave(
  "Eval-Compare-PresenceAS.pdf",
  ggarrange(
    ggarrange(make_presences_rel(FALSE), as_presences_abs, ncol = 2),
    the_legend,
    nrow = 2, heights = c(7, 1)
  ),
  units="cm", width=19, height=6
)

### - presences per prefix len overall

make_compare_ratio <- function (would_you_like_fries_with_that) {
  ggplot(data = dat_huge, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
    geom_bar(position = "fill", show.legend = would_you_like_fries_with_that) +
    scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
    scale_y_continuous(name = "% of Nodes", labels = scales::percent) +
    theme(legend.direction='horizontal', legend.box='horizontal') +
    theme(legend.text=element_text(size=10)) +
    guides(fill = guide_legend(nrow = 1)) +
    scale_fill_presences()
}

compare_abs <- ggplot(data = dat_huge, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(show.legend = FALSE) +
  scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()

the_legend <- as_ggplot(get_legend(make_compare_ratio(TRUE)))

ggsave(
    "Eval-Compare-PresenceAll.pdf",
    ggarrange(
      ggarrange(make_compare_ratio(FALSE), compare_abs, ncol = 2),
      the_legend,
      nrow = 2, heights = c(7, 1)
    ),
    units="cm", width=19, height=6
)


make_compare_ratio <- function (would_you_like_fries_with_that) {
  ggplot(data = dat_huge, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
    geom_bar(position = "fill", show.legend = would_you_like_fries_with_that) +
    scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
    scale_y_continuous(name = "% of Nodes", labels = scales::percent) +
    theme(legend.direction='horizontal', legend.box='horizontal') +
    theme(legend.text=element_text(size=10)) +
    guides(fill = guide_legend(nrow = 1)) +
    scale_fill_presences()
}

### - same but only leaves

dat_leaves <- dat_huge |>
  filter(eval_present == 'SplitNode' | ref_present != 'SplitNode')

# ok nearly all are leaves on at least one side soo useless

dat_leaves <- dat_huge |>
  filter(eval_present == 'KeptLeaf' | eval_present == 'KeepCandidate')

ggplot(data = dat_leaves, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar() +
  scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()

### AS1120 has the most samesies of them all, what happened there?

as1120 <- dat_huge |>
  filter(asn == 1120) |>
  filter(str_detect(net, "^2001:628:453"))
as1120_presences <- ggplot(data = as1120, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(show.legend = FALSE) +
  scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length - AS1120") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()

uni_presences <- ggplot(data = dat_uni, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(show.legend = FALSE) +
  scale_x_continuous(name = "Prefix Length - University") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()

ggsave(
  "Eval-Compare-PresenceExamples.pdf",
  ggarrange(
    ggarrange(uni_presences, as1120_presences, ncol = 2),
    the_legend, # reuse from previous, yolo
    nrow = 2, heights = c(7, 1)
  ),
  units="cm", width=19, height=6
)
                     
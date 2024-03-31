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
PRESENCES_LABELS = c(
  "Different",
  "CandidateDifferent",
  "CandidateSame",
  "Same",
  "AT-11 only", # EvalOnly (inverted sadly, but this is how it gets written)
  "AT-10 only" # ReferenceOnly
)

scale_fill_presences <- function() {
  scale_fill_manual(values=PRESENCES_COLORS, breaks=PRESENCES_ORDER, labels=PRESENCES_LABELS, name=NULL)
  scale_fill_manual(values=PRESENCES_COLORS, breaks=PRESENCES_ORDER, labels=PRESENCES_LABELS, name=NULL)
}

### - AS presences per prefix len

make_presences_rel <- function(with_legend) {
  ggplot(data = dat_as, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
    geom_bar(position = "fill", show.legend = with_legend, color = "black") +
    scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
    scale_y_continuous(name = "% of Nodes", labels = scales::percent) +
    theme(legend.direction='horizontal', legend.box='horizontal') +
    theme(legend.text=element_text(size=10)) +
    guides(fill = guide_legend(nrow = 1)) +
    scale_fill_presences()
}

as_presences_abs <- ggplot(data = dat_as, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(show.legend = FALSE, color = "black") +
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

make_compare_ratio <- function (base, would_you_like_fries_with_that) {
  ggplot(data = base, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
    geom_bar(position = "fill", show.legend = would_you_like_fries_with_that, color = "black") +
    scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
    scale_y_continuous(name = "% of Nodes", labels = scales::percent) +
    theme(legend.direction='horizontal', legend.box='horizontal') +
    theme(legend.text=element_text(size=10)) +
    guides(fill = guide_legend(nrow = 1)) +
    scale_fill_presences()
}

compare_abs <- ggplot(data = dat_huge, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(show.legend = FALSE, color = "black") +
  scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()

the_legend <- as_ggplot(get_legend(make_compare_ratio(dat_huge, TRUE)))

ggsave(
    "Eval-Compare-PresenceAll.pdf",
    ggarrange(
      ggarrange(make_compare_ratio(dat_huge, FALSE), compare_abs, ncol = 2),
      the_legend,
      nrow = 2, heights = c(7, 1)
    ),
    units="cm", width=19, height=6
)

### - same but only leaves

dat_leaves <- dat_huge |>
  filter(eval_present == 'SplitNode' | ref_present != 'SplitNode')

# ok nearly all are leaves on at least one side soo useless

dat_leaves <- dat_huge |>
  filter(eval_present == 'KeptLeaf' | eval_present == 'KeepCandidate')

ggplot(data = dat_leaves, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(color = "black") +
  scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()

### Same but excluding MediumSame splits

only_medium_same <- dat_huge |>
  filter(!str_detect(ref_class, "^MediumSame") & !str_detect(eval_class, "^MediumSame"))
nrow(only_medium_same) # only 10k :(

compare_oms <- make_compare_ratio(only_medium_same, FALSE)
compare_oms_abs <- ggplot(data = only_medium_same, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(show.legend = FALSE, color = "black") +
  scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()

the_legend2 <- as_ggplot(get_legend(make_compare_ratio(only_medium_same, TRUE)))

ggsave(
  "Eval-Compare-PresenceAllNoOms.pdf",
  ggarrange(
    ggarrange(compare_oms, compare_oms_abs, ncol = 2),
    the_legend2,
    nrow = 2, heights = c(7, 1)
  ),
  units="cm", width=19, height=6
)

### Look at very similar ASes

samesies_as <- dat_huge |>
  filter(compare_presence == "Same") |>
  group_by(asn) |>
  summarize(n = n())
# 40980 has the most
# 2a01:aea0:dd3::/48 is one BGP root there, the rest looks similar

all_as_counts <- dat_huge |>
  group_by(asn) |>
  summarize(n = n()) |>
  filter(n > 1)




same_analysis_total <- merge(samesies_as, all_as_counts, by="asn") |>
  mutate(same_ratio = n.x / n.y) |>
  filter(same_ratio != 1 | (asn == 39555 | asn == 51066))
# manual cleanup of AS with multiple roots that were not split
# 8387 - merged back up (AT-11)
# 8666
# 30971 - merged back up (AT-11)
# 35052
# 39555 - legit
# 42354
# 44133
# 51066 - legit
# 199216
# 200986
# 201240 - merged back up (AT-11)
# 202928
# 205202
# 207869
# 210775
# 211763

hist(same_analysis_total$same_ratio)

similar_ases <- ggplot(data = same_analysis_total, aes(x = same_ratio)) + 
  geom_histogram(aes(y = after_stat(count / sum(count))), color = "black", fill = "#d29630", bins = 10) + 
  labs(title = NULL, x = "Similarity distribution (AS with multiple nodes)") +
  coord_cartesian(ylim = c(0, 0.25)) + 
  scale_y_continuous(name = "% AS", labels = scales::percent, breaks = seq(0, 0.25, 0.05)) +
  geom_vline(aes(xintercept=median(same_ratio)), color="#34b0a7")
similar_ases

as40980 <- dat_huge |>
  filter(asn == 40980) |>
  filter(str_detect(net, "^2a01:aea0:dd3"))
as40980_presences <- ggplot(data = as40980, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(show.legend = FALSE, color = "black") +
  scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length - Top Similar BGP Root") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()
as40980_presences

#as1120 <- dat_huge |>
#  filter(asn == 1120) |>
#  filter(str_detect(net, "^2001:628:453"))
#as1120_presences <- ggplot(data = as1120, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
#  geom_bar(show.legend = FALSE) +
#  scale_x_continuous(breaks=NET_BREAKS, name = "Prefix Length - Top Similar BGP Root") +
#  scale_y_continuous(name = "# of Nodes") +
#  scale_fill_presences()

uni_presences <- ggplot(data = dat_uni, aes(x = net_len, fill = factor(compare_presence, PRESENCES_ORDER))) +
  geom_bar(show.legend = FALSE, color = "black") +
  scale_x_continuous(name = "Prefix Length - University") +
  scale_y_continuous(name = "# of Nodes") +
  scale_fill_presences()

ggsave(
  "Eval-Compare-PresenceExamples.pdf",
  ggarrange(
    ggarrange(uni_presences, similar_ases, ncol = 2),
    the_legend, # reuse from previous, yolo
    nrow = 2, heights = c(7, 1)
  ),
  units="cm", width=19, height=6
)
                     

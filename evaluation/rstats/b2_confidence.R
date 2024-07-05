library(readr)
library(ggplot2)
library(gridExtra)
library(egg)
library(ggpubr)
library(dplyr)


at10 <- read_csv("confidence_dist_at10.csv")
at11 <- read_csv("confidence_dist_at11.csv")

percent_of <- function(label, mine, base) {
  paste0(label, " (none: ", formatC(100 * ((nrow(base)-nrow(mine))/nrow(base)), format = "f", digits = 0), "%)")
}

###

at10_binned <- at10 |>
  mutate(bin = floor(confidence/10) * 10 + 5) |> # +5 to center bin on chart
  group_by(bin) |>
  summarise(all_nodes = sum(all_nodes))

at10_med <- at10 %>%
  summarise(median_confidence = median(rep(confidence, all_nodes))) # 100..

all_nodes_at10 <- ggplot(data = at10_binned, aes(x = bin, y = all_nodes)) +
  geom_bar(stat="identity", fill = "#9967bc", color = "black") +
  labs(title = NULL, x = "Confidence Metric - AT-10") +
  #geom_vline(aes(xintercept=at10_med), color="#34b0a7") +
  #annotate("text", x=39.5, y=50000, label=paste0("med ", at10_med), angle=90, color="#34b0a7", size=3) +
  scale_y_log10(name = "# Prefix Tree Nodes (all)", breaks=c(10, 100, 1000, 25000, 130000)) +
  scale_x_continuous(breaks=c(0, 50, 100, 150, 200, 255)) +
  coord_cartesian(ylim = c(1, 131000))

at11_binned <- at11 |>
  mutate(bin = floor(confidence/10) * 10 + 5) |>
  group_by(bin) |>
  summarise(all_nodes = sum(all_nodes))

at11_med <- at10 %>%
  summarise(median_confidence = median(rep(confidence, all_nodes))) # 100..

all_nodes_at11 <- ggplot(data = at11_binned, aes(x = bin, y = all_nodes)) +
  geom_bar(stat="identity", fill = "#d19cf5", color = "black") +
  labs(title = NULL, x = "Confidence Metric - AT-11") +
 # geom_vline(aes(xintercept=at11_med), color="#34b0a7") +
 # annotate("text", x=46, y=50000, label=paste0("med ", at11_med), angle=90, color="#34b0a7", size=3) +
  scale_y_log10(name = NULL, breaks=c(10, 100, 1000, 25000, 130000)) +
  scale_x_continuous(breaks=c(0, 50, 100, 150, 200, 255)) +
  coord_cartesian(ylim = c(1, 131000))

all_nodes <- ggarrange(all_nodes_at10, all_nodes_at11, ncol = 2)
ggsave("Eval-Confidence-PerNodeCount.pdf", all_nodes, units="cm", width=19, height=6)

### - boring!

at10_nona_ned <- at10 |>
  filter(!is.na(split_nodes)) |>
  filter(split_nodes > 0)
at10_binned2 <- at10_nona_ned |>
  mutate(bin = floor(confidence/10) * 10 + 5) |> # +5 to center bin on chart
  group_by(bin) |>
  summarise(split_nodes=sum(split_nodes))
at10_med2 <- round(median(at10_nona_ned$split_nodes), digits=0)

split_at10 <- ggplot(data = at10_binned2, aes(x = bin, y = split_nodes)) +
  geom_bar(stat="identity", fill = "#9967bc", color = "black") +
  labs(title = NULL, x = "Confidence Metric - AT-10 - Split") +
  geom_vline(aes(xintercept=at10_med), color="#34b0a7") +
  annotate("text", x=39.5, y=50000, label=paste0("med ", at10_med2), angle=90, color="#34b0a7", size=3) +
  scale_y_log10(name = "# Prefix Tree Splits", breaks=c(10, 100, 1000, 25000, 130000)) +
  scale_x_continuous(breaks=c(0, 50, 100, 150, 200, 255)) +
  coord_cartesian(ylim = c(1, 131000))
split_at10

### laevs targetable

at10_nona_ned2 <- at10 |>
  filter(!is.na(targetable_leaf_nodes)) |>
  filter(targetable_leaf_nodes > 0)
at10_binned2 <- at10_nona_ned2 |>
  mutate(bin = floor(confidence/10) * 10 + 5) |> # +5 to center bin on chart
  group_by(bin) |>
  summarise(targetable_leaf_nodes=sum(targetable_leaf_nodes))

at10_med2 <- at10_nona_ned2 %>%
  summarise(median_confidence = median(rep(confidence, targetable_leaf_nodes))) # 100..
at10_med22 <- at10_med2[["median_confidence"]]

leaves_at10 <- ggplot(data = at10_binned2, aes(x = bin, y = targetable_leaf_nodes)) +
  geom_bar(stat="identity", fill = "#9967bc", color = "black") +
  labs(title = NULL, x = "Confidence Metric - AT-10") +
  geom_vline(aes(xintercept=at10_med22), color="#34b0a7") +
  annotate("text", x=120, y=23000, label=paste0("med ", at10_med22), angle=90, color="#34b0a7", size=3) +
  scale_y_continuous(name = "# Targetable Leaves", breaks=c(0, 7500, 26250)) +
  scale_x_continuous(breaks=c(0, 50, 100, 150, 200, 255)) +
  coord_cartesian(ylim = c(1, 27000))

at11_nona_ned2 <- at11 |>
  filter(!is.na(targetable_leaf_nodes)) |>
  filter(targetable_leaf_nodes > 0)
at11_binned2 <- at11_nona_ned2 |>
  mutate(bin = floor(confidence/10) * 10 + 5) |> # +5 to center bin on chart
  group_by(bin) |>
  summarise(targetable_leaf_nodes=sum(targetable_leaf_nodes))
at11_med2 <- at11_nona_ned2 %>%
  summarise(median_confidence = median(rep(confidence, targetable_leaf_nodes))) # 100..
at11_med22 <- at11_med2[["median_confidence"]]

leaves_at11 <- ggplot(data = at11_binned2, aes(x = bin, y = targetable_leaf_nodes)) +
  geom_bar(stat="identity", fill = "#9967bc", color = "black") +
  labs(title = NULL, x = "Confidence Metric - AT-11") +
  geom_vline(aes(xintercept=at11_med22), color="#34b0a7") +
  annotate("text", x=120, y=23000, label=paste0("med ", at11_med22), angle=90, color="#34b0a7", size=3) +
  scale_y_continuous(name = "# Targetable Leaves", breaks=c(0, 7500, 26250)) +
  scale_x_continuous(breaks=c(0, 50, 100, 150, 200, 255)) +
  coord_cartesian(ylim = c(1, 27000))

all_nodes <- ggarrange(leaves_at10, leaves_at11, ncol = 2)
ggsave("Eval-Confidence-PerLeafCount.pdf", all_nodes, units="cm", width=19, height=6)

### leaves splitable

at10_nona_ned2 <- at10 |>
  filter(!is.na(targetable_leaf_nodes)) |>
  filter(targetable_leaf_nodes > 0)
at10_binned2 <- at10_nona_ned2 |>
  mutate(bin = floor(confidence/10) * 10 + 5) |> # +5 to center bin on chart
  group_by(bin) |>
  summarise(targetable_leaf_nodes=sum(targetable_leaf_nodes))

at10_med2 <- at10_nona_ned2 %>%
  summarise(median_confidence = median(rep(confidence, targetable_leaf_nodes))) # 100..
at10_med22 <- at10_med2[["median_confidence"]]

leaves_at10 <- ggplot(data = at10_binned2, aes(x = bin, y = targetable_leaf_nodes)) +
  geom_bar(stat="identity", fill = "#9967bc", color = "black") +
  labs(title = NULL, x = "Confidence Metric - AT-10") +
  geom_vline(aes(xintercept=at10_med22), color="#34b0a7") +
  annotate("text", x=120, y=23000, label=paste0("med ", at10_med22), angle=90, color="#34b0a7", size=3) +
  scale_y_continuous(name = "# Targetable Leaves", breaks=c(0, 7500, 26250)) +
  scale_x_continuous(breaks=c(0, 50, 100, 150, 200, 255)) +
  coord_cartesian(ylim = c(1, 27000))

at11_nona_ned2 <- at11 |>
  filter(!is.na(targetable_leaf_nodes)) |>
  filter(targetable_leaf_nodes > 0)
at11_binned2 <- at11_nona_ned2 |>
  mutate(bin = floor(confidence/10) * 10 + 5) |> # +5 to center bin on chart
  group_by(bin) |>
  summarise(targetable_leaf_nodes=sum(targetable_leaf_nodes))
at11_med2 <- at11_nona_ned2 %>%
  summarise(median_confidence = median(rep(confidence, targetable_leaf_nodes))) # 100..
at11_med22 <- at11_med2[["median_confidence"]]

leaves_at11 <- ggplot(data = at11_binned2, aes(x = bin, y = targetable_leaf_nodes)) +
  geom_bar(stat="identity", fill = "#9967bc", color = "black") +
  labs(title = NULL, x = "Confidence Metric - AT-11") +
  geom_vline(aes(xintercept=at11_med22), color="#34b0a7") +
  annotate("text", x=115, y=23000, label=paste0("med ", at11_med22), angle=90, color="#34b0a7", size=3) +
  scale_y_continuous(name = "# Targetable Leaves", breaks=c(0, 7500, 26250)) +
  scale_x_continuous(breaks=c(0, 50, 100, 150, 200, 255)) +
  coord_cartesian(ylim = c(1, 27000))

all_nodes <- ggarrange(leaves_at10, leaves_at11, ncol = 2)
ggsave("Eval-Confidence-PerLeafCount.pdf", all_nodes, units="cm", width=19, height=6)



#!/usr/bin/env python3
import sys
from typing import Union, List, Literal

from dashboard_builder import (
    Layout,
    timeseries_panel,
    target,
    template,
    Expr,
    Stat,
    expr_sum_rate,
    expr_aggr_func,
    expr_avg,
    heatmap_panel,
    yaxis,
    expr_operator,
    expr_max,
    DATASOURCE,
)
from grafanalib import formatunits as UNITS, _gen
from grafanalib.core import (
    Dashboard,
    Templating,
    Template,
    Annotations,
    RowPanel,
    Panel,
    HeatmapColor,
    Tooltip,
    GRAPH_TOOLTIP_MODE_SHARED_CROSSHAIR,
    Target,
)


# todo: do something with this metrics
# tycho_core_last_mc_block_applied
# tycho_core_last_sc_block_applied
# tycho_core_last_sc_block_seqno
# tycho_core_last_sc_block_utime


def heatmap_color_warm() -> HeatmapColor:
    return HeatmapColor()


def generate_legend_format(labels: List[str]) -> str:
    """
    Generate a legend format string based on the provided labels.

    Args:
    labels (List[str]): A list of label strings.

    Returns:
    str: A legend format string including instance and provided labels with keys.
    """
    legend_format = "{{instance}}"
    for label in labels:
        key = label.split("=")[0]  # Extract the key part before '='
        legend_format += f" {key}:{{{{{key}}}}}"
    return legend_format


def create_gauge_panel(
    expr: Union[str, List[Union[str, Expr]], Expr],
    title: str,
    unit_format=UNITS.NUMBER_FORMAT,
    labels=[],
    legend_format: str | None = None,
) -> Panel:
    if isinstance(expr, str):
        expr = [Expr(metric=expr, label_selectors=labels)]
    elif isinstance(expr, list):
        expr = [
            Expr(metric=e, label_selectors=labels) if isinstance(e, str) else e
            for e in expr
        ]
    elif isinstance(expr, Expr):
        expr = [expr]
    else:
        raise TypeError(
            "expr must be a string, a list of strings, or a list of Expr objects."
        )

    if legend_format is None:
        legend_format = generate_legend_format(labels)

    targets = [target(e, legend_format=legend_format) for e in expr]

    return timeseries_panel(
        title=title,
        targets=targets,
        unit=unit_format,
    )


def create_counter_panel(
    expr: Union[str | Expr, List[Union[str, Expr]]],
    title: str,
    unit_format: str = UNITS.NUMBER_FORMAT,
    labels_selectors: List[str] = [],
    legend_format: str | None = None,
    by_labels: list[str] = ["instance"],
) -> Panel:
    """
    Create a counter panel for visualization.

    Args:
        expr (Union[str, List[Union[str, Expr]]]): Expression or list of expressions to visualize.
        title (str): Title of the panel.
        unit_format (str, optional): Format for the unit display. Defaults to UNITS.NUMBER_FORMAT.
        labels_selectors (List[str], optional): List of label selectors. Defaults to an empty list.
        legend_format (str | None, optional): Format for the legend. If None, it's generated automatically. Defaults to None.
        by_labels (list[str], optional): Labels to group by. Defaults to ["instance"].

    Returns:
        Panel: A timeseries panel object.
    """
    if legend_format is None:
        legend_format = generate_legend_format(labels_selectors)

    if isinstance(expr, str):
        targets = [
            target(
                expr_sum_rate(
                    expr, label_selectors=labels_selectors, by_labels=by_labels
                ),
                legend_format=legend_format,
            )
        ]
    elif isinstance(expr, list):
        if all(isinstance(e, str) for e in expr):
            targets = [
                target(
                    expr_sum_rate(
                        e, label_selectors=labels_selectors, by_labels=by_labels
                    ),
                    legend_format=legend_format,
                )
                for e in expr
            ]
        elif all(isinstance(e, Expr) for e in expr):
            targets = [target(e, legend_format=legend_format) for e in expr]
        else:
            raise ValueError("List elements must be all strings or all Expr objects.")
    elif isinstance(expr, Expr):
        targets = [target(expr, legend_format=legend_format)]
    else:
        raise TypeError(
            "expr must be a string, a list of strings, or a list of Expr objects."
        )

    return timeseries_panel(
        title=title,
        targets=targets,
        unit=unit_format,
    )


def create_percent_panel(
    metric1: str,
    metric2: str,
    title: str,
    group_by_labels: List[str] = ["instance"],
    label_selectors: List[str] = [],
    unit_format: str = UNITS.PERCENT_FORMAT,
) -> Panel:
    """
    create a panel showing the percentage of metric1 to metric2, grouped by specified labels.

    Args:
        metric1 (str): The first metric (numerator).
        metric2 (str): The second metric (denominator).
        title (str): Title of the panel.
        group_by_labels (List[str]): Labels to group by and match on.
        label_selectors (List[str]): Additional label selectors for both metrics.
        unit_format (str, optional): Format for the unit display. defaults to UNITS.PERCENT_FORMAT.

    Returns:
        Panel: A timeseries panel object showing the percentage.
    """
    expr1 = expr_sum_rate(
        metric1, label_selectors=label_selectors, by_labels=group_by_labels
    )
    expr2 = expr_sum_rate(
        metric2, label_selectors=label_selectors, by_labels=group_by_labels
    )

    percent_expr = expr_operator(expr_operator(expr1, "/", expr2), "*", "100")

    legend_format = "{{" + "}} - {{".join(group_by_labels) + "}}"

    percent_target = target(percent_expr, legend_format=legend_format)

    return timeseries_panel(title=title, targets=[percent_target], unit=unit_format)


def create_heatmap_panel(
    metric_name,
    title,
    unit_format=yaxis(UNITS.SECONDS),
    labels=[],
) -> Panel:
    return heatmap_panel(
        title,
        f"{metric_name}_bucket",
        yaxis=unit_format,
        color=heatmap_color_warm(),
        tooltip=Tooltip(),
        label_selectors=labels,
        rate_interval="10s",  # todo: update this if scrape interval changes
    )


# Type alias for accepted quantiles
ACCEPTED_QUANTILES = {"0", "0.5", "0.9", "0.95", "0.99", "0.999", "1"}
AcceptedQuantile = Literal["0", "0.5", "0.9", "0.95", "0.99", "0.999", "1"]


def create_heatmap_quantile_panel(
    metric_name: str,
    title: str,
    unit_format: str = UNITS.NUMBER_FORMAT,
    quantile: AcceptedQuantile = "0.95",
) -> Panel:
    """
    Create a heatmap quantile panel for the given metric.

    Args:
        metric_name (str): Name of the metric to visualize.
        title (str): Title of the panel.
        unit_format (str, optional): Unit format for the panel. Defaults to UNITS.NUMBER_FORMAT.
        quantile (AcceptedQuantile, optional): Quantile to use (as an integer 0-100). Defaults to 95.

    Returns:
        Panel: A configured grafanalib Panel object.

    Raises:
        ValueError: If the quantile is not one of the accepted values.
    """

    if quantile not in ACCEPTED_QUANTILES:
        raise ValueError(f"Quantile must be one of {ACCEPTED_QUANTILES}")

    legend_format = f"{{{{instance}}}} p{quantile}"
    quantile_expr = f'quantile="{quantile}"'

    return timeseries_panel(
        title=title,
        targets=[
            target(
                expr=Expr(metric_name, label_selectors=[quantile_expr]),
                legend_format=legend_format,
            )
        ],
        unit=unit_format,
    )


def create_row(
    name: str, metrics, repeat: str | None = None, collapsed=True
) -> RowPanel:
    layout = Layout(name, repeat=repeat, collapsed=collapsed)
    for i in range(0, len(metrics), 2):
        chunk = metrics[i : i + 2]
        layout.row(chunk)
    return layout.row_panel


def blockchain_stats() -> RowPanel:
    def expr_aggr_avg_rate(metric: str) -> Expr:
        rate = expr_sum_rate(metric)
        return expr_aggr_func(f"{rate}", "avg", "avg_over_time", by_labels=[]).extra(
            default_label_selectors=[]
        )

    first_row = [
        timeseries_panel(
            targets=[
                target(expr_aggr_avg_rate("tycho_bc_txs_total"), legend_format="avg")
            ],
            title="Transactions Rate",
            unit="tx/s",
            legend_display_mode="hidden",
        ),
        timeseries_panel(
            targets=[
                target(
                    expr_aggr_avg_rate("tycho_bc_ext_msgs_total"),
                    legend_format="received",
                ),
                target(
                    expr_aggr_avg_rate("tycho_do_collate_msgs_error_count_ext"),
                    legend_format="failed",
                ),
                target(
                    expr_aggr_avg_rate("tycho_do_collate_msgs_skipped_count_ext"),
                    legend_format="skipped",
                ),
                target(
                    expr_aggr_avg_rate("tycho_do_collate_ext_msgs_expired_count"),
                    legend_format="expired",
                ),
            ],
            title="External Messages Rate",
            unit="msg/s",
            legend_display_mode="hidden",
        ),
        Stat(
            targets=[
                Target(
                    expr=f"""{
                        expr_max(
                            "tycho_last_applied_block_seqno",
                            label_selectors=['workchain="-1"'],
                            by_labels=[],
                        )
                    }""",
                    legendFormat="Last Applied MC Block",
                    instant=True,
                    datasource=DATASOURCE,
                ),
                Target(
                    expr=f"""{
                        expr_max(
                            "tycho_last_processed_to_anchor_id",
                            label_selectors=['workchain="-1"'],
                            by_labels=[],
                        )
                    }""",
                    legendFormat="Last Used Anchor",
                    instant=True,
                    datasource=DATASOURCE,
                ),
            ],
            graphMode="area",
            textMode="value_and_name",
            reduceCalc="lastNotNull",
            format=UNITS.NONE_FORMAT,
        ),
    ]

    second_row = [
        timeseries_panel(
            targets=[
                target(
                    expr_avg(
                        "tycho_storage_store_block_data_size",
                        label_selectors=['quantile="0.5"'],
                        by_labels=[],
                    ),
                    legend_format="P50",
                ),
                target(
                    expr_avg(
                        "tycho_storage_store_block_data_size",
                        label_selectors=['quantile="0.999"'],
                        by_labels=[],
                    ),
                    legend_format="P99",
                ),
            ],
            title="Block Data Size",
            unit=UNITS.BYTES,
            legend_display_mode="hidden",
        ),
        timeseries_panel(
            targets=[
                target(
                    expr_aggr_func(
                        "tycho_do_collate_blocks_count",
                        "avg",
                        "rate",
                        label_selectors=['workchain=~"$workchain"'],
                        by_labels=["workchain"],
                    ),
                    legend_format="{{workchain}}",
                )
            ],
            title="Blocks Rate",
            unit="blocks/s",
            legend_display_mode="hidden",
        ),
        timeseries_panel(
            targets=[
                target(
                    expr_aggr_func(
                        "tycho_mempool_engine_current_round",
                        "avg",
                        "rate",
                        by_labels=[],
                    ),
                    legend_format="rate",
                )
            ],
            title="Mempool Rounds Rate",
            unit="rounds/s",
            legend_display_mode="hidden",
        ),
    ]

    layout = Layout("Stats", repeat=None, collapsed=True)
    layout.row(first_row)
    layout.row(second_row)
    return layout.row_panel


def core_bc() -> RowPanel:
    metrics = [
        timeseries_panel(
            targets=[
                target(
                    'timestamp(up{instance=~"$instance"}) - tycho_core_last_mc_block_utime{instance=~"$instance"}',
                    legend_format="{{instance}}",
                )
            ],
            unit=UNITS.SECONDS,
            title="Mc block processing lag",
        ),
        create_gauge_panel(
            "tycho_shard_blocks_count_in_last_master_block",
            "Shard blocks count in master block",
            labels=['workchain=~"$workchain"'],
        ),
        create_gauge_panel(
            "tycho_last_applied_block_seqno",
            "Last applied block seqno",
            labels=['workchain=~"$workchain"'],
        ),
        create_gauge_panel(
            "tycho_last_processed_to_anchor_id",
            "Last processed to anchor",
            labels=['workchain=~"$workchain"'],
        ),
        create_counter_panel("tycho_bc_txs_total", "Number of transactions over time"),
        create_counter_panel(
            "tycho_bc_ext_msgs_total", "Number of external messages over time"
        ),
        create_counter_panel("tycho_bc_msgs_total", "Number of all messages over time"),
        create_heatmap_quantile_panel(
            "tycho_bc_total_gas_used", "Total gas used per block", quantile="1"
        ),
        create_counter_panel(
            "tycho_bc_contract_deploy_total", "Number of contract deployments over time"
        ),
        create_counter_panel(
            "tycho_bc_contract_delete_total", "Number of contract deletions over time"
        ),
        create_heatmap_quantile_panel(
            "tycho_bc_in_msg_count",
            "Number of inbound messages per block",
            quantile="0.999",
        ),
        create_heatmap_quantile_panel(
            "tycho_bc_out_msg_count",
            "Number of outbound messages per block",
            quantile="0.999",
        ),
        create_heatmap_quantile_panel(
            "tycho_bc_out_in_msg_ratio",
            "Out/In message ratio per block",
            quantile="0.999",
        ),
        create_heatmap_quantile_panel(
            "tycho_bc_out_msg_acc_ratio",
            "Out message/Account ratio per block",
            quantile="0.999",
        ),
        create_gauge_panel(
            "tycho_do_collate_accounts_per_block",
            "Number of accounts per block",
            labels=['workchain=~"$workchain"'],
        ),
        create_gauge_panel(
            "tycho_shard_accounts_count",
            "Number of accounts in state",
            labels=['workchain=~"$workchain"'],
        ),
        create_gauge_panel(
            "tycho_do_collate_added_accounts_count",
            "Number of added accounts in block",
            labels=['workchain=~"$workchain"'],
        ),
        create_gauge_panel(
            "tycho_do_collate_removed_accounts_count",
            "Number of removed accounts in block",
            labels=['workchain=~"$workchain"'],
        ),
        # todo: pie chart?
        create_heatmap_quantile_panel(
            "tycho_bc_software_version",
            "Software version per block",
            quantile="0.999",
        ),
    ]
    return create_row("Blockchain", metrics)


def net_traffic() -> RowPanel:
    legend_format = "{{instance}} - {{service}}"
    by_labels = ["service", "instance"]
    metrics = [
        create_counter_panel(
            "tycho_private_overlay_tx",
            "Private overlay traffic sent",
            UNITS.BYTES_SEC_IEC,
            legend_format=legend_format,
            by_labels=by_labels,
        ),
        create_counter_panel(
            "tycho_private_overlay_rx",
            "Private overlay traffic received",
            UNITS.BYTES_SEC_IEC,
            legend_format=legend_format,
            by_labels=by_labels,
        ),
        create_counter_panel(
            "tycho_public_overlay_tx",
            "Public overlay traffic sent",
            UNITS.BYTES_SEC_IEC,
            legend_format=legend_format,
            by_labels=by_labels,
        ),
        create_counter_panel(
            "tycho_public_overlay_rx",
            "Public overlay traffic received",
            UNITS.BYTES_SEC_IEC,
            legend_format=legend_format,
            by_labels=by_labels,
        ),
        create_counter_panel(
            "tycho_rpc_broadcast_external_message_tx_bytes_total",
            "RPC broadcast external message traffic sent",
            UNITS.BYTES_SEC_IEC,
        ),
        create_counter_panel(
            "tycho_rpc_broadcast_external_message_rx_bytes_total",
            "RPC broadcast external message traffic received",
            UNITS.BYTES_SEC_IEC,
        ),
    ]
    return create_row("network: Traffic", metrics)


def core_blockchain_rpc_general() -> RowPanel:
    metrics = [
        create_gauge_panel(
            "tycho_core_overlay_client_validators_to_resolve",
            "Number of validators to resolve",
        ),
        create_gauge_panel(
            "tycho_core_overlay_client_resolved_validators",
            "Number of resolved validators",
        ),
        create_gauge_panel(
            "tycho_core_overlay_client_target_validators",
            "Number of selected broadcast targets",
        ),
        create_heatmap_panel(
            "tycho_core_overlay_client_validator_ping_time", "Time to ping validator"
        ),
        create_gauge_panel(
            expr=[
                "tycho_broadcast_timeout",
            ],
            title="Broadcast Timeout",
            unit_format=UNITS.SECONDS,
            legend_format="{{instance}} - {{kind}}",
        ),
        create_gauge_panel(
            "tycho_core_overlay_client_neighbour_score",
            "Current score per peer",
        ),
        create_counter_panel(
            "tycho_core_overlay_client_neighbour_total_requests", "Number of total requests per peer"
        ),
        create_counter_panel(
            "tycho_core_overlay_client_neighbour_failed_requests", "Number of failed requests per peer"
        ),
    ]
    return create_row("blockchain: RPC - General Stats", metrics)


def core_blockchain_rpc_per_method_stats() -> RowPanel:
    methods = [
        "getNextKeyBlockIds",
        "getBlockFull",
        "getBlockDataChunk",
        "getNextBlockFull",
        "getKeyBlockProof",
        "getArchiveInfo",
        "getArchiveChunk",
        "getPersistentShardStateInfo",
        "getPersistentQueueStateInfo",
        "getPersistentShardStateChunk",
        "getPersistentQueueStateChunk",
    ]

    counter_panels = [
        create_counter_panel(
            expr="tycho_blockchain_rpc_method_time_count",
            title=f"Blockchain RPC {method} calls/s",
            labels_selectors=[f'method="{method}"'],
            legend_format="{{instance}}",
        )
        for method in methods
    ]

    heatmap_panels = [
        create_heatmap_panel(
            "tycho_blockchain_rpc_method_time",
            f"Blockchain RPC {method} time",
            labels=[f'method="{method}"'],
        )
        for method in methods
    ]

    return create_row("blockchain: RPC - Method Stats", counter_panels + heatmap_panels)


def util_reclaimer() -> RowPanel:
    # Queue length = sum by (instance) (enqueued) - sum by (instance) (dropped)
    # Use raw timeseries_panel to avoid the label selector issue
    queue_panel = timeseries_panel(
        title="Reclaimer queue length",
        targets=[
            target(
                "sum by (instance) (tycho_delayed_drop_enqueued{instance=~\"$instance\"}) - sum by (instance) (tycho_delayed_drop_dropped{instance=~\"$instance\"})",
                legend_format="{{instance}}"
            )
        ],
        unit=UNITS.NUMBER_FORMAT,
    )

    metrics = [
        queue_panel,
        create_heatmap_panel(
            "tycho_delayed_drop_time",
            "Reclaimer drop time",
        ),
        create_counter_panel(
            "tycho_delayed_drop_enqueued",
            "Reclaimer enqueue rate",
        ),
        create_counter_panel(
            "tycho_delayed_drop_dropped",
            "Reclaimer dequeue rate",
        ),
    ]
    return create_row("util: Reclaimer", metrics)


def net_conn_manager() -> RowPanel:
    metrics = [
        create_heatmap_panel(
            "tycho_net_conn_out_time", "Time taken to establish an outgoing connection"
        ),
        create_heatmap_panel(
            "tycho_net_conn_in_time", "Time taken to establish an incoming connection"
        ),
        create_counter_panel(
            "tycho_net_conn_out_total",
            "Number of established outgoing connections over time",
        ),
        create_counter_panel(
            "tycho_net_conn_in_total",
            "Number of established incoming connections over time",
        ),
        create_counter_panel(
            "tycho_net_conn_out_fail_total",
            "Number of failed outgoing connections over time",
        ),
        create_counter_panel(
            "tycho_net_conn_in_fail_total",
            "Number of failed incoming connections over time",
        ),
        create_gauge_panel(
            "tycho_net_conn_active", "Number of currently active connections"
        ),
        create_gauge_panel(
            "tycho_net_conn_pending", "Number of currently pending connections"
        ),
        create_gauge_panel(
            "tycho_net_conn_partial", "Number of currently half-resolved connections"
        ),
        create_gauge_panel(
            "tycho_net_conn_pending_dials",
            "Number of currently pending connectivity checks",
        ),
        create_gauge_panel(
            "tycho_net_active_peers", "Number of currently active peers"
        ),
        create_gauge_panel("tycho_net_known_peers", "Number of currently known peers"),
    ]
    return create_row("network: Connection Manager", metrics)


def net_request_handler() -> RowPanel:
    metrics = [
        create_heatmap_panel(
            "tycho_net_in_queries_time", "Duration of incoming queries handlers"
        ),
        create_heatmap_panel(
            "tycho_net_in_messages_time", "Duration of incoming messages handlers"
        ),
        create_counter_panel(
            "tycho_net_in_queries_total", "Number of incoming queries over time"
        ),
        create_counter_panel(
            "tycho_net_in_messages_total", "Number of incoming messages over time"
        ),
        create_counter_panel(
            "tycho_net_in_requests_rejected_total",
            "Number of rejected incoming messages over time",
        ),
        create_gauge_panel(
            "tycho_net_req_handlers", "Current number of incoming request handlers"
        ),
        create_gauge_panel(
            "tycho_net_req_handlers_per_peer",
            "Current number of incoming request handlers per peer",
        ),
    ]
    return create_row("network: Request Handler", metrics)


def net_peer() -> RowPanel:
    metrics = [
        create_heatmap_panel(
            "tycho_net_out_queries_time", "Duration of outgoing queries"
        ),
        create_heatmap_panel(
            "tycho_net_out_messages_time", "Duration of outgoing messages"
        ),
        create_counter_panel(
            "tycho_net_out_queries_total", "Number of outgoing queries over time"
        ),
        create_counter_panel(
            "tycho_net_out_messages_total", "Number of outgoing messages over time"
        ),
        create_gauge_panel(
            "tycho_net_out_queries", "Current number of outgoing queries"
        ),
        create_gauge_panel(
            "tycho_net_out_messages", "Current number of outgoing messages"
        ),
    ]
    return create_row("network: Peers", metrics)


def net_dht() -> RowPanel:
    metrics = [
        create_counter_panel(
            "tycho_net_dht_in_req_total", "Number of incoming DHT requests over time"
        ),
        create_counter_panel(
            "tycho_net_dht_in_req_fail_total",
            "Number of failed incoming DHT requests over time",
        ),
        create_counter_panel(
            "tycho_net_dht_in_req_with_peer_info_total",
            "Number of incoming DHT requests with peer info over time",
        ),
        create_counter_panel(
            "tycho_net_dht_in_req_find_node_total",
            "Number of incoming DHT FindNode requests over time",
        ),
        create_counter_panel(
            "tycho_net_dht_in_req_find_value_total",
            "Number of incoming DHT FindValue requests over time",
        ),
        create_counter_panel(
            "tycho_net_dht_in_req_get_node_info_total",
            "Number of incoming DHT GetNodeInfo requests over time",
        ),
        create_counter_panel(
            "tycho_net_dht_in_req_store_value_total",
            "Number of incoming DHT Store requests over time",
        ),
    ]
    return create_row("network: DHT", metrics)


def core_block_strider() -> RowPanel:
    metrics = [
        create_heatmap_panel(
            "tycho_core_process_strider_step_time",
            "Time to process block strider step",
        ),
        create_heatmap_panel(
            "tycho_core_provider_cleanup_time",
            "Time to cleanup block providers",
        ),
        create_heatmap_panel(
            "tycho_core_download_mc_block_time", "Masterchain block downloading time"
        ),
        create_heatmap_panel(
            "tycho_core_prepare_mc_block_time", "Masterchain block preparing time"
        ),
        create_heatmap_panel(
            "tycho_core_process_mc_block_time", "Masterchain block processing time"
        ),
        create_heatmap_panel(
            "tycho_core_download_sc_block_time", "Shard block downloading time"
        ),
        create_heatmap_panel(
            "tycho_core_prepare_sc_block_time",
            "Shard block preparing time",
        ),
        create_heatmap_panel(
            "tycho_core_process_sc_block_time",
            "Shard block processing time",
        ),
        create_heatmap_panel(
            "tycho_core_download_sc_blocks_time",
            "Total time to download all shard blocks",
        ),
        create_heatmap_panel(
            "tycho_core_process_sc_blocks_time",
            "Total time to process all shard blocks",
        ),
        create_heatmap_panel(
            "tycho_core_state_applier_prepare_block_time",
            "Time to prepare block by ShardStateApplier",
        ),
        create_heatmap_panel(
            "tycho_core_state_applier_handle_block_time",
            "Time to handle block by ShardStateApplier",
        ),
        create_heatmap_panel(
            "tycho_core_archive_handler_prepare_block_time",
            "Time to prepare block by ArchiveHandler",
        ),
        create_heatmap_panel(
            "tycho_core_archive_handler_handle_block_time",
            "Time to handle block by ArchiveHandler",
        ),
        create_heatmap_panel(
            "tycho_core_subscriber_handle_state_time",
            "Total time to handle state by all subscribers",
        ),
        create_heatmap_panel(
            "tycho_core_subscriber_handle_archive_time",
            "Total time to handle archive by all subscribers",
        ),
        create_heatmap_panel(
            "tycho_core_apply_block_time_high",
            "Time to apply and save block state",
            labels=['workchain=~"$workchain"'],
        ),
        create_heatmap_panel(
            "tycho_core_apply_block_in_mem_time_high",
            "Time to apply block state",
        ),
        create_heatmap_panel(
            "tycho_core_metrics_subscriber_handle_block_time",
            "Time to handle block by MetricsSubscriber",
        ),
        create_heatmap_panel(
            "tycho_core_check_block_proof_time", "Check block proof time"
        ),
        create_counter_panel(
            "tycho_core_ps_subscriber_saved_persistent_states_count",
            "Saved persistent states",
        ),
    ]
    return create_row("block strider: Core Metrics", metrics)


def storage() -> RowPanel:
    metrics = [
        create_heatmap_panel(
            "tycho_storage_load_cell_time", "Time to load cell from storage"
        ),
        create_counter_panel(
            expr_sum_rate("tycho_storage_load_cell_time_count"),
            "Number of load_cell calls",
            UNITS.OPS_PER_SEC,
        ),
        create_heatmap_panel(
            "tycho_storage_get_cell_from_rocksdb_time", "Time to load cell from RocksDB"
        ),
        create_counter_panel(
            expr_sum_rate("tycho_storage_get_cell_from_rocksdb_time_count"),
            "Number of cache missed cell loads",
            UNITS.OPS_PER_SEC,
        ),
        timeseries_panel(
            title="Storage Cache Hit Rate",
            targets=[
                target(
                    expr='(1 - (sum(rate(tycho_storage_get_cell_from_rocksdb_time_count{instance=~"$instance"}[$__rate_interval])) by (instance) / sum(rate(tycho_storage_load_cell_time_count{instance=~"$instance"}[$__rate_interval])) by (instance))) * 100',
                    legend_format="Hit Rate",
                )
            ],
            unit=UNITS.PERCENT_FORMAT,
        ),
        create_gauge_panel(
            "tycho_storage_state_max_epoch",
            "Max known state root cell epoch",
            unit_format="Seqno"
        ),
        create_gauge_panel(
            "tycho_storage_raw_cells_cache_size",
            "Raw cells cache size",
            UNITS.BYTES_IEC,
        ),
        create_heatmap_quantile_panel(
            "tycho_storage_store_block_data_size",
            "Block data size",
            UNITS.BYTES_IEC,
            "0.999",
        ),
        create_heatmap_quantile_panel(
            "tycho_storage_cell_count",
            "Number of new cells from merkle update",
            quantile="0.999",
        ),
        create_heatmap_panel(
            "tycho_storage_state_update_time_high",
            "Time to write state update to rocksdb",
        ),
        create_heatmap_panel(
            "tycho_storage_state_store_time",
            "Time to store single root with rocksdb write etc",
        ),
        create_heatmap_panel(
            "tycho_storage_cell_in_mem_store_time_high",
            "Time to store cell without write",
        ),
        create_heatmap_panel(
            "tycho_storage_cell_gc_lock_store_time_high",
            "Time to wait gc mutex during store",
        ),
        create_heatmap_panel(
            "tycho_storage_batch_write_time_high", "Time to write merge in write batch"
        ),
        create_heatmap_panel(
            "tycho_storage_batch_write_parallel_time_high",
            "Time to write merge in write batch in parallel",
        ),
        create_heatmap_quantile_panel(
            "tycho_storage_state_update_size_bytes",
            "State update size",
            UNITS.BYTES,
            "0.999",
        ),
        create_heatmap_quantile_panel(
            "tycho_storage_state_update_size_predicted_bytes",
            "Predicted state update size",
            UNITS.BYTES,
            "0.999",
        ),
        create_heatmap_panel(
            "tycho_storage_state_store_time", "Time to store state with cell traversal"
        ),
        create_heatmap_panel("tycho_gc_states_time", "Time to garbage collect state"),
        timeseries_panel(
            targets=[
                target(
                    'tycho_core_last_mc_block_seqno{instance=~"$instance"} - on(instance, job) tycho_gc_states_seqno{instance=~"$instance"}',
                    legend_format="{{instance}}",
                )
            ],
            unit="States",
            title="States GC lag",
        ),
        timeseries_panel(
            title="States GC seqno guard",
            targets=[
                target(
                    "tycho_min_ref_mc_seqno{instance=~\"$instance\"} > 0",
                    legend_format="{{instance}}"
                )
            ],
            unit="States",
        ),
        timeseries_panel(
            title="States GC safe range",
            targets=[
                target(
                    "tycho_min_ref_mc_seqno{instance=~\"$instance\"} - tycho_gc_states_seqno{instance=~\"$instance\"}",
                    legend_format="{{instance}}"
                )
            ],
            unit="States",
        ),
        create_gauge_panel(
            "tycho_core_mc_blocks_gc_lag", "Blocks GC lag", unit_format="Blocks"
        ),
        create_gauge_panel("tycho_core_blocks_gc_tail_len", "GC diffs tail len"),
        create_heatmap_panel(
            "tycho_storage_move_into_archive_time", "Time to move into archive"
        ),
        create_heatmap_panel(
            "tycho_storage_commit_archive_time", "Time to commit archive"
        ),
        create_heatmap_panel(
            "tycho_storage_split_block_data_time", "Time to split block data"
        ),
        create_gauge_panel(
            "tycho_storage_cells_tree_cache_size", "Cells tree cache size"
        ),
        create_counter_panel(
            "tycho_compaction_keeps", "Number of not deleted cells during compaction"
        ),
        create_counter_panel(
            "tycho_compaction_removes", "Number of deleted cells during compaction"
        ),
        create_counter_panel(
            "tycho_storage_state_gc_count", "number of deleted states during gc"
        ),
        create_counter_panel(
            "tycho_storage_state_gc_cells_count", "number of deleted cells during gc"
        ),
        create_heatmap_panel(
            "tycho_storage_state_gc_time_high", "time spent to gc single root"
        ),
        create_heatmap_panel(
            "tycho_storage_cell_in_mem_remove_time_high",
            "Time to remove cell without write",
        ),
        create_heatmap_panel(
            "tycho_storage_cell_gc_lock_remove_time_high",
            "Time to wait gc mutex during remove",
        ),
        create_heatmap_panel(
            "tycho_storage_load_block_data_time", "Time to load block data"
        ),
        create_counter_panel(
            "tycho_storage_load_block_data_time_count",
            "Number of load_block_data calls",
        ),
        create_percent_panel(
            "tycho_storage_block_cache_hit_total",
            "tycho_storage_load_block_total",
            "Block cache hit ratio",
        ),
    ]
    return create_row("Storage", metrics)


def allocator_stats() -> RowPanel:
    metrics = [
        create_gauge_panel(
            "jemalloc_allocated_bytes", "Allocated Bytes", UNITS.BYTES_IEC
        ),
        create_gauge_panel("jemalloc_active_bytes", "Active Bytes", UNITS.BYTES_IEC),
        create_gauge_panel(
            "jemalloc_metadata_bytes", "Metadata Bytes", UNITS.BYTES_IEC
        ),
        create_gauge_panel(
            "jemalloc_resident_bytes", "Resident Bytes", UNITS.BYTES_IEC
        ),
        create_gauge_panel("jemalloc_mapped_bytes", "Mapped Bytes", UNITS.BYTES_IEC),
        create_gauge_panel(
            "jemalloc_retained_bytes", "Retained Bytes", UNITS.BYTES_IEC
        ),
        create_gauge_panel("jemalloc_dirty_bytes", "Dirty Bytes", UNITS.BYTES_IEC),
        create_gauge_panel(
            "jemalloc_fragmentation_bytes", "Fragmentation Bytes", UNITS.BYTES_IEC
        ),
    ]
    return create_row("Allocator Stats", metrics)


def rayon_stats() -> RowPanel:
    metrics = [
        create_heatmap_panel(
            "tycho_rayon_lifo_threads", "LIFO Threads", yaxis(UNITS.NUMBER_FORMAT)
        ),
        create_heatmap_panel(
            "tycho_rayon_fifo_threads", "FIFO Threads", yaxis(UNITS.NUMBER_FORMAT)
        ),
        create_heatmap_panel(
            "tycho_rayon_lifo_task_time", "LIFO Task Time", yaxis(UNITS.SECONDS)
        ),
        create_heatmap_panel(
            "tycho_rayon_fifo_task_time", "FIFO Task Time", yaxis(UNITS.SECONDS)
        ),
        create_heatmap_panel(
            "tycho_rayon_lifo_queue_time", "LIFO Queue Time", yaxis(UNITS.SECONDS)
        ),
        create_heatmap_panel(
            "tycho_rayon_fifo_queue_time", "FIFO Queue Time", yaxis(UNITS.SECONDS)
        ),
    ]
    return create_row("Rayon Stats", metrics)


def quic_network_panels() -> list[RowPanel]:
    """Panels for QUIC network performance monitoring"""
    common_labels = ['peer_id=~"$peer_id"', 'peer_addr=~"$remote_addr"']
    legend = "{{instance}} ->  {{peer_addr}}"
    by_labels = ["instance", "peer_addr"]

    def counter_with_defaults(metric, title, unit=UNITS.PACKETS_SEC):
        return create_counter_panel(
            metric,
            title,
            labels_selectors=common_labels,
            legend_format=legend,
            by_labels=by_labels,
            unit_format=unit,
        )

    def gauge_with_defaults(metric, title, unit=UNITS.PACKETS_SEC):
        return create_gauge_panel(
            metric, title, labels=common_labels, legend_format=legend, unit_format=unit
        )

    # Core network metrics
    network_perf_panels = [
        gauge_with_defaults(
            "tycho_network_connection_rtt_ms", "RTT", UNITS.MILLI_SECONDS
        ),
        gauge_with_defaults(
            "tycho_network_connection_cwnd", "Congestion Window", "packets"
        ),
        counter_with_defaults(
            "tycho_network_connection_invalid_messages", "Invalid Messages"
        ),
        counter_with_defaults(
            "tycho_network_connection_congestion_events", "Congestion Events"
        ),
        create_percent_panel(
            "tycho_network_connection_lost_packets",
            "tycho_network_connection_sent_packets",
            "Packet Loss Rate",
            label_selectors=common_labels,
        ),
    ]

    # Throughput metrics
    data_movement_panels = [
        counter_with_defaults(
            "tycho_network_connection_rx_bytes", "Bytes Received/s", UNITS.BYTES_IEC
        ),
        counter_with_defaults(
            "tycho_network_connection_tx_bytes", "Bytes Sent/s", UNITS.BYTES_IEC
        ),
    ]

    def frame_panel_pair(frame_type, display_name):
        return [
            counter_with_defaults(
                f"tycho_network_connection_rx_{frame_type}", f"RX {display_name}"
            ),
            counter_with_defaults(
                f"tycho_network_connection_tx_{frame_type}", f"TX {display_name}"
            ),
        ]

    protocol_panels = []
    for frame_type, name in [
        ("stream", "Stream Frames"),
        ("acks", "ACK Frames"),
        ("datagram", "Datagram Frames"),
        ("max_data", "Max Data Frames"),
        ("max_stream_data", "Max Stream Data"),
        ("ping", "Ping Frames"),
        ("crypto", "Crypto Handshake"),
    ]:
        protocol_panels.extend(frame_panel_pair(frame_type, name))

    error_panels = [
        counter_with_defaults(
            [
                "tycho_network_connection_rx_connection_close",
                "tycho_network_connection_tx_connection_close",
            ],
            "Connection Close Frames",
        ),
        counter_with_defaults(
            [
                "tycho_network_connection_rx_reset_stream",
                "tycho_network_connection_tx_reset_stream",
            ],
            "Reset Stream Frames",
        ),
        counter_with_defaults(
            [
                "tycho_network_connection_rx_stop_sending",
                "tycho_network_connection_tx_stop_sending",
            ],
            "Stop Sending Frames",
        ),
        counter_with_defaults(
            [
                "tycho_network_connection_rx_data_blocked",
                "tycho_network_connection_tx_data_blocked",
            ],
            "Data Blocked Frames",
        ),
        counter_with_defaults(
            [
                "tycho_network_connection_rx_stream_data_blocked",
                "tycho_network_connection_tx_stream_data_blocked",
            ],
            "Stream Data Blocked Frames",
        ),
    ]

    return [
        create_row("quinn: Network Performance", network_perf_panels),
        create_row("quinn: Data Movement", data_movement_panels),
        create_row("quinn: Protocol Operations", protocol_panels),
        create_row("quinn: Error Conditions", error_panels),
    ]


def templates() -> Templating:
    return Templating(
        list=[
            Template(
                name="source",
                query="prometheus",
                type="datasource",
            ),
            template(
                name="instance",
                query="label_values(tycho_net_known_peers, instance)",
                data_source="${source}",
                hide=0,
                regex=None,
                multi=True,
                include_all=True,
                all_value=".*",
            ),
            template(
                name="workchain",
                query="label_values(tycho_do_collate_block_time_diff,workchain)",
                data_source="${source}",
                hide=0,
                regex=None,
                multi=True,
                include_all=True,
                all_value=".*",
            ),
            template(
                name="partition",
                query="label_values(tycho_do_collate_processed_upto_int_ranges,par_id)",
                data_source="${source}",
                hide=0,
                regex=None,
                multi=True,
                include_all=True,
                all_value=".*",
            ),
            template(
                name="kind",
                query="label_values(tycho_mempool_verifier_verify,kind)",
                data_source="${source}",
                hide=0,
                regex=None,
                multi=True,
                include_all=True,
                all_value=".*",
            ),
            template(
                name="method",
                query="label_values(tycho_jrpc_request_time_bucket,method)",
                data_source="${source}",
                hide=0,
                regex=None,
                multi=True,
                include_all=True,
                all_value=".*",
            ),
            template(
                name="peer_id",
                query="label_values(tycho_network_connection_rtt_ms, peer_id)",
                data_source="${source}",
                hide=0,
                regex=None,
                multi=True,
                include_all=True,
                all_value=".*",
            ),
            template(
                name="remote_addr",
                query="label_values(tycho_network_connection_rtt_ms, addr)",
                data_source="${source}",
                hide=0,
                regex=None,
                multi=True,
                include_all=True,
                all_value=".*",
            ),
        ]
    )


dashboard = Dashboard(
    "Tycho Node Metrics",
    templating=templates(),
    refresh="30s",
    panels=[
        blockchain_stats(),
        core_bc(),
        core_block_strider(),
        core_blockchain_rpc_general(),
        core_blockchain_rpc_per_method_stats(),
        storage(),
        net_traffic(),
        net_conn_manager(),
        net_request_handler(),
        net_peer(),
        net_dht(),
        *quic_network_panels(),
        util_reclaimer(),
        allocator_stats(),
        rayon_stats(),
    ],
    annotations=Annotations(),
    uid="cdlaji62a1b0gb",
    version=9,
    schemaVersion=14,
    graphTooltip=GRAPH_TOOLTIP_MODE_SHARED_CROSSHAIR,
    timezone="browser",
).auto_panel_ids()


# open file as stream
if len(sys.argv) > 1:
    stream = open(sys.argv[1], "w")
else:
    stream = sys.stdout
# write dashboard to file
_gen.write_dashboard(dashboard, stream)

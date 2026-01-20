#!/usr/bin/env python3
import pandas as pd
import matplotlib
matplotlib.use('Agg')  # 使用非交互式后端
import matplotlib.pyplot as plt
import argparse
import os
import sys
from datetime import datetime

def load_csv_data(csv_file):
    if not os.path.exists(csv_file):
        print(f"Error: CSV file '{csv_file}' does not exist")
        sys.exit(1)
    
    try:
        df = pd.read_csv(csv_file)
        print(f"Loaded {len(df)} data points from {csv_file}")
        return df
    except Exception as e:
        print(f"Error reading CSV file: {e}")
        sys.exit(1)

def plot_throughput(df, output_prefix='batch_benchmark'):
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    
    fig, axes = plt.subplots(2, 2, figsize=(16, 12))
    fig.suptitle('Batch Benchmark Throughput Analysis', fontsize=16, fontweight='bold')
    
    # Batch throughput
    ax1 = axes[0, 0]
    ax1.plot(df['elapsed_seconds'], df['batches_per_second'], 'b-', linewidth=2, label='Batches/sec')
    ax1.set_xlabel('Time (seconds)')
    ax1.set_ylabel('Batches per second')
    ax1.set_title('Batch Throughput Over Time')
    ax1.grid(True, alpha=0.3)
    ax1.legend()
    
    # Entry throughput
    ax2 = axes[0, 1]
    ax2.plot(df['elapsed_seconds'], df['entries_per_second'], 'g-', linewidth=2, label='Entries/sec')
    ax2.set_xlabel('Time (seconds)')
    ax2.set_ylabel('Entries per second')
    ax2.set_title('Entry Throughput Over Time')
    ax2.grid(True, alpha=0.3)
    ax2.legend()
    
    # Bandwidth
    ax3 = axes[1, 0]
    ax3.plot(df['elapsed_seconds'], df['bytes_per_second'] / (1024 * 1024), 'r-', linewidth=2, label='MB/sec')
    ax3.set_xlabel('Time (seconds)')
    ax3.set_ylabel('Bandwidth (MB/sec)')
    ax3.set_title('Write Bandwidth Over Time')
    ax3.grid(True, alpha=0.3)
    ax3.legend()
    
    # Dirty pages
    ax4 = axes[1, 1]
    ax4.plot(df['elapsed_seconds'], df['dirty_ratio_percent'], 'orange', linewidth=2, label='Dirty Page Ratio %')
    ax4_twin = ax4.twinx()
    ax4_twin.plot(df['elapsed_seconds'], df['dirty_pages_kb'] / 1024, 'purple', linewidth=1.5, linestyle='--', label='Dirty Pages (MB)')
    ax4.set_xlabel('Time (seconds)')
    ax4.set_ylabel('Dirty Page Ratio (%)', color='orange')
    ax4_twin.set_ylabel('Dirty Pages (MB)', color='purple')
    ax4.set_title('Memory Dirty Pages Over Time')
    ax4.grid(True, alpha=0.3)
    ax4.tick_params(axis='y', labelcolor='orange')
    ax4_twin.tick_params(axis='y', labelcolor='purple')
    
    lines1, labels1 = ax4.get_legend_handles_labels()
    lines2, labels2 = ax4_twin.get_legend_handles_labels()
    ax4.legend(lines1 + lines2, labels1 + labels2, loc='upper right')
    
    plt.tight_layout()
    
    output_file = f"{output_prefix}_throughput_{timestamp}.png"
    plt.savefig(output_file, dpi=150, bbox_inches='tight')
    print(f"Saved throughput chart to: {output_file}")
    plt.close()

def plot_cumulative(df, output_prefix='batch_benchmark'):
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    
    fig, axes = plt.subplots(1, 3, figsize=(18, 6))
    fig.suptitle('Batch Benchmark Cumulative Statistics', fontsize=16, fontweight='bold')
    
    ax1 = axes[0]
    ax1.plot(df['elapsed_seconds'], df['total_batches'], 'b-', linewidth=2)
    ax1.set_xlabel('Time (seconds)')
    ax1.set_ylabel('Total Batches')
    ax1.set_title('Cumulative Batches')
    ax1.grid(True, alpha=0.3)
    
    ax2 = axes[1]
    ax2.plot(df['elapsed_seconds'], df['total_entries'], 'g-', linewidth=2)
    ax2.set_xlabel('Time (seconds)')
    ax2.set_ylabel('Total Entries')
    ax2.set_title('Cumulative Entries')
    ax2.grid(True, alpha=0.3)
    
    ax3 = axes[2]
    ax3.plot(df['elapsed_seconds'], df['total_bytes'] / (1024 * 1024), 'r-', linewidth=2)
    ax3.set_xlabel('Time (seconds)')
    ax3.set_ylabel('Total Bytes (MB)')
    ax3.set_title('Cumulative Bytes Written')
    ax3.grid(True, alpha=0.3)
    
    plt.tight_layout()
    
    output_file = f"{output_prefix}_cumulative_{timestamp}.png"
    plt.savefig(output_file, dpi=150, bbox_inches='tight')
    print(f"Saved cumulative chart to: {output_file}")
    plt.close()

def plot_statistics_summary(df, output_prefix='batch_benchmark'):
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    
    fig, axes = plt.subplots(2, 2, figsize=(16, 12))
    fig.suptitle('Batch Benchmark Statistics Summary', fontsize=16, fontweight='bold')
    
    throughput_data = df['entries_per_second'].dropna()
    bandwidth_data = df['bytes_per_second'].dropna() / (1024 * 1024)
    
    # Throughput distribution
    ax1 = axes[0, 0]
    ax1.hist(throughput_data, bins=50, color='green', alpha=0.7, edgecolor='black')
    ax1.set_xlabel('Entries/sec')
    ax1.set_ylabel('Frequency')
    ax1.set_title('Entry Throughput Distribution')
    ax1.grid(True, alpha=0.3, axis='y')
    ax1.axvline(throughput_data.mean(), color='red', linestyle='--', linewidth=2, label=f'Mean: {throughput_data.mean():.0f}')
    ax1.legend()
    
    # Bandwidth distribution
    ax2 = axes[0, 1]
    ax2.hist(bandwidth_data, bins=50, color='red', alpha=0.7, edgecolor='black')
    ax2.set_xlabel('Bandwidth (MB/sec)')
    ax2.set_ylabel('Frequency')
    ax2.set_title('Write Bandwidth Distribution')
    ax2.grid(True, alpha=0.3, axis='y')
    ax2.axvline(bandwidth_data.mean(), color='blue', linestyle='--', linewidth=2, label=f'Mean: {bandwidth_data.mean():.2f}')
    ax2.legend()
    
    # Box plot
    ax3 = axes[1, 0]
    ax3.boxplot([throughput_data, bandwidth_data], labels=['Entries/sec', 'MB/sec'])
    ax3.set_ylabel('Value')
    ax3.set_title('Performance Metrics Box Plot')
    ax3.grid(True, alpha=0.3, axis='y')
    
    # Moving average
    ax4 = axes[1, 1]
    ax4.plot(df['elapsed_seconds'], df['entries_per_second'], 'g-', alpha=0.5, linewidth=1, label='Raw Data')
    ax4.plot(df['elapsed_seconds'], df['entries_per_second'].rolling(window=10, min_periods=1).mean(), 'b-', linewidth=2, label='Moving Avg (10)')
    ax4.plot(df['elapsed_seconds'], df['entries_per_second'].rolling(window=30, min_periods=1).mean(), 'r-', linewidth=2, label='Moving Avg (30)')
    ax4.set_xlabel('Time (seconds)')
    ax4.set_ylabel('Entries/sec')
    ax4.set_title('Throughput with Moving Average')
    ax4.grid(True, alpha=0.3)
    ax4.legend()
    
    plt.tight_layout()
    
    output_file = f"{output_prefix}_statistics_{timestamp}.png"
    plt.savefig(output_file, dpi=150, bbox_inches='tight')
    print(f"Saved statistics chart to: {output_file}")
    plt.close()

def print_summary(df):
    print("\n" + "="*80)
    print("Benchmark Summary")
    print("="*80)
    
    throughput = df['entries_per_second'].dropna()
    bandwidth = df['bytes_per_second'].dropna() / (1024 * 1024)
    
    print(f"\nDuration: {df['elapsed_seconds'].max():.2f} seconds")
    print(f"Total Batches: {df['total_batches'].max():,}")
    print(f"Total Entries: {df['total_entries'].max():,}")
    print(f"Total Bytes: {df['total_bytes'].max() / (1024**3):.2f} GB")
    
    print(f"\nThroughput Statistics (entries/sec):")
    print(f"  Mean: {throughput.mean():.0f}")
    print(f"  Median:  {throughput.median():.0f}")
    print(f"  Min:     {throughput.min():.0f}")
    print(f"  Max:     {throughput.max():.0f}")
    print(f"  Std Dev: {throughput.std():.0f}")
    
    print(f"\nBandwidth Statistics (MB/sec):")
    print(f"  Mean: {bandwidth.mean():.2f}")
    print(f"  Median:  {bandwidth.median():.2f}")
    print(f"  Min:     {bandwidth.min():.2f}")
    print(f"  Max:     {bandwidth.max():.2f}")
    print(f"  Std Dev: {bandwidth.std():.2f}")
    
    print(f"\nMemory Statistics:")
    print(f"  Max Dirty Pages: {df['dirty_pages_kb'].max() / 1024:.2f} MB")
    print(f"  Max Dirty Page Ratio: {df['dirty_ratio_percent'].max():.2f}%")
    
    print("="*80 + "\n")

def main():
    parser = argparse.ArgumentParser(description='Visualize batch benchmark results (non-interactive)')
    parser.add_argument('--file', '-f', required=True, help='CSV file to visualize')
    parser.add_argument('--output', '-o', default='batch_benchmark', help='Output file prefix')
    parser.add_argument('--summary-only', '-s', action='store_true', help='Only print summary, do not generate charts')
    parser.add_argument('--no-throughput', action='store_true', help='Skip throughput chart')
    parser.add_argument('--no-cumulative', action='store_true', help='Skip cumulative chart')
    parser.add_argument('--no-statistics', action='store_true', help='Skip statistics chart')
    
    args = parser.parse_args()
    
    df = load_csv_data(args.file)
    print_summary(df)
    
    if not args.summary_only:
        if not args.no_throughput:
            plot_throughput(df, args.output)
        
        if not args.no_cumulative:
            plot_cumulative(df, args.output)
        
        if not args.no_statistics:
            plot_statistics_summary(df, args.output)
        
        print("\nAll charts generated successfully!")

if __name__ == '__main__':
    main()
import { useStreams } from '../hooks';
import type { Stream } from '../types';

function formatDuration(startedAt: string): string {
  const start = new Date(startedAt);
  const now = new Date();
  const diff = now.getTime() - start.getTime();

  const hours = Math.floor(diff / (1000 * 60 * 60));
  const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
  const seconds = Math.floor((diff % (1000 * 60)) / 1000);

  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
}

function getStreamStatus(status: Stream['status']): string {
  if ('Active' in status) return 'Active';
  if ('Inactive' in status) return 'Inactive';
  if ('Error' in status) return `Error: ${status.Error}`;
  return 'Unknown';
}

function StreamCard({ stream }: { stream: Stream }) {
  const bitrateMbps = (stream.bitrate.total_bitrate / 1_000_000).toFixed(2);

  return (
    <div className="stream-card">
      <div className="stream-header">
        <h3>{stream.stream_key}</h3>
        <span className={`status-badge ${getStreamStatus(stream.status).toLowerCase()}`}>
          {getStreamStatus(stream.status)}
        </span>
      </div>

      <div className="stream-details">
        <div className="detail">
          <span className="label">App:</span>
          <span className="value">{stream.app_name}</span>
        </div>
        <div className="detail">
          <span className="label">Publisher:</span>
          <span className="value">{stream.publisher_addr}</span>
        </div>
        <div className="detail">
          <span className="label">Duration:</span>
          <span className="value">{formatDuration(stream.started_at)}</span>
        </div>
        <div className="detail">
          <span className="label">Bitrate:</span>
          <span className="value">{bitrateMbps} Mbps</span>
        </div>
        <div className="detail">
          <span className="label">Packets:</span>
          <span className="value">{stream.bitrate.packets_received.toLocaleString()}</span>
        </div>
      </div>
    </div>
  );
}

export default function StreamsPage() {
  const { data: streams, isLoading, error } = useStreams();

  if (isLoading) {
    return <div className="loading">Loading streams...</div>;
  }

  if (error) {
    return <div className="error">Error loading streams: {error.message}</div>;
  }

  if (!streams || streams.length === 0) {
    return (
      <div className="empty-state">
        <h2>No Active Streams</h2>
        <p>Connect an RTMP client to rtmp://localhost:1935/live/your-stream-key</p>
      </div>
    );
  }

  return (
    <div className="streams-page">
      <h1>Active Streams ({streams.length})</h1>
      <div className="streams-grid">
        {streams.map(stream => (
          <StreamCard key={stream.id} stream={stream} />
        ))}
      </div>
    </div>
  );
}

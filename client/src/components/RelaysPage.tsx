import { useState } from 'react';
import { useRelays, useStreams, useCreateRelay, useDeleteRelay, useStartRelay, useStopRelay } from '../hooks';
import type { Relay } from '../types';

function getRelayStatus(status: Relay['status']): string {
  if ('Idle' in status) return 'Idle';
  if ('Connecting' in status) return 'Connecting';
  if ('Active' in status) return 'Active';
  if ('Stopped' in status) return 'Stopped';
  if ('Error' in status) return `Error: ${status.Error}`;
  return 'Unknown';
}

function RelayCard({ relay }: { relay: Relay }) {
  const { data: streams } = useStreams();
  const startRelay = useStartRelay();
  const stopRelay = useStopRelay();
  const deleteRelay = useDeleteRelay();

  const handleStart = () => {
    if (streams && streams.length > 0) {
      const streamId = streams[0].id; // For now, use first stream
      startRelay.mutate({ id: relay.id, data: { stream_id: streamId } });
    } else {
      alert('No active streams to relay');
    }
  };

  const handleStop = () => {
    stopRelay.mutate(relay.id);
  };

  const handleDelete = () => {
    if (confirm(`Delete relay "${relay.name}"?`)) {
      deleteRelay.mutate(relay.id);
    }
  };

  const status = getRelayStatus(relay.status);
  const isActive = status === 'Active' || status === 'Connecting';
  const canStart = status === 'Idle' || status === 'Stopped';

  return (
    <div className="relay-card">
      <div className="relay-header">
        <h3>{relay.name}</h3>
        <span className={`status-badge ${status.toLowerCase()}`}>
          {status}
        </span>
      </div>

      <div className="relay-details">
        <div className="detail">
          <span className="label">Target:</span>
          <span className="value truncate">{relay.target_url}</span>
        </div>
        {relay.stream_id && (
          <div className="detail">
            <span className="label">Stream ID:</span>
            <span className="value truncate">{relay.stream_id}</span>
          </div>
        )}
        {relay.bytes_sent > 0 && (
          <div className="detail">
            <span className="label">Sent:</span>
            <span className="value">
              {(relay.bytes_sent / 1024 / 1024).toFixed(2)} MB
            </span>
          </div>
        )}
      </div>

      <div className="relay-actions">
        {canStart && (
          <button
            onClick={handleStart}
            disabled={startRelay.isPending}
            className="btn-start"
          >
            Start
          </button>
        )}
        {isActive && (
          <button
            onClick={handleStop}
            disabled={stopRelay.isPending}
            className="btn-stop"
          >
            Stop
          </button>
        )}
        <button
          onClick={handleDelete}
          disabled={deleteRelay.isPending}
          className="btn-delete"
        >
          Delete
        </button>
      </div>
    </div>
  );
}

export default function RelaysPage() {
  const { data: relays, isLoading, error } = useRelays();
  const createRelay = useCreateRelay();
  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState('');
  const [targetUrl, setTargetUrl] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (name && targetUrl) {
      createRelay.mutate(
        { name, target_url: targetUrl },
        {
          onSuccess: () => {
            setName('');
            setTargetUrl('');
            setShowForm(false);
          },
        }
      );
    }
  };

  if (isLoading) {
    return <div className="loading">Loading relays...</div>;
  }

  if (error) {
    return <div className="error">Error loading relays: {error.message}</div>;
  }

  return (
    <div className="relays-page">
      <div className="page-header">
        <h1>Relays ({relays?.length || 0})</h1>
        <button
          onClick={() => setShowForm(!showForm)}
          className="btn-primary"
        >
          {showForm ? 'Cancel' : '+ Add Relay'}
        </button>
      </div>

      {showForm && (
        <form onSubmit={handleSubmit} className="relay-form">
          <div className="form-group">
            <label htmlFor="name">Relay Name</label>
            <input
              id="name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="YouTube Live"
              required
            />
          </div>
          <div className="form-group">
            <label htmlFor="target">Target URL</label>
            <input
              id="target"
              type="text"
              value={targetUrl}
              onChange={(e) => setTargetUrl(e.target.value)}
              placeholder="rtmp://a.rtmp.youtube.com/live2/YOUR-KEY"
              required
            />
          </div>
          <button
            type="submit"
            disabled={createRelay.isPending}
            className="btn-submit"
          >
            {createRelay.isPending ? 'Creating...' : 'Create Relay'}
          </button>
        </form>
      )}

      {!relays || relays.length === 0 ? (
        <div className="empty-state">
          <h2>No Relays Configured</h2>
          <p>Create a relay to forward streams to other RTMP servers</p>
        </div>
      ) : (
        <div className="relays-grid">
          {relays.map(relay => (
            <RelayCard key={relay.id} relay={relay} />
          ))}
        </div>
      )}
    </div>
  );
}

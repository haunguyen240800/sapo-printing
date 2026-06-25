export default function TestPage() {
  return (
    <div style={{ padding: '20px' }}>
      <h1>Test Page</h1>
      <p>If you can see this, the app is working!</p>
      <button onClick={() => alert('Button clicked!')}>
        Click me
      </button>
    </div>
  );
}

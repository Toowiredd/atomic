import { Toaster } from 'sonner';
import { Layout } from './components/layout';
import { useEmbeddingEvents } from './hooks';

function App() {
  // Initialize embedding event listener
  useEmbeddingEvents();

  return (
    <>
      <Toaster
        position="bottom-right"
        theme="dark"
        toastOptions={{
          className: 'atomic-toast',
          duration: 5000,
        }}
      />
      <Layout />
    </>
  );
}

export default App;


import { PrinterConfigForm } from './components/printer/PrinterConfigForm';
import { PrintJobDashboard } from './components/print-job/PrintJobDashboard';

function App() {
  return (
    <div>
      <PrintJobDashboard />
      <PrinterConfigForm />
    </div>
  );
}

export default App;

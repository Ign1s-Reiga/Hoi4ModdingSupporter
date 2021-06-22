using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Forms;

namespace Hoi4ModdingSupporter
{
    public partial class MainWindow : Form
    {
        private string filePath;
        private string fileName;

        public MainWindow()
        {
            InitializeComponent();
        }

        private void newProjectToolMenuItem_Click(object sender, EventArgs e)
        {

        }

        private void openProjectToolMenuItem_Click(object sender, EventArgs e)
        {
            FolderBrowserDialog dialog = new();
            
            if (dialog.ShowDialog() == DialogResult.Cancel)
            {
                return;
            }

            this.filePath = dialog.SelectedPath;
            dialog.Dispose();

            DirectoryInfo info = new DirectoryInfo(this.filePath);
            this.fileName = info.Name;
        }

        private void editToolMenuItem_Click(object sender, EventArgs e)
        {

        }

        private void versionToolMenuItem_Click(object sender, EventArgs e)
        {
            VersionPopup version = new VersionPopup("alpha-1");
            version.Show();
        }

        private void endToolMenuItem_Click(object sender, EventArgs e)
        {
            this.Close();
        }


    }
}

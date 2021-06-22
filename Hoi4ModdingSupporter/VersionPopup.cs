using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Drawing;
using System.Text;
using System.Windows.Forms;

namespace Hoi4ModdingSupporter
{
    public partial class VersionPopup : Form
    {
        private String version;

        public VersionPopup(String version)
        {
            this.version = version;
            InitializeComponent(this.version);
        }
    }
}

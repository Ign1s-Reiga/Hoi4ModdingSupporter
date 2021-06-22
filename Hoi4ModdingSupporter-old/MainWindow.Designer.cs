
namespace Hoi4ModdingSupporter
{
    partial class MainWindow
    {
        /// <summary>
        ///  Required designer variable.
        /// </summary>
        private System.ComponentModel.IContainer components = null;

        /// <summary>
        ///  Clean up any resources being used.
        /// </summary>
        /// <param name="disposing">true if managed resources should be disposed; otherwise, false.</param>
        protected override void Dispose(bool disposing)
        {
            if (disposing && (components != null))
            {
                components.Dispose();
            }
            base.Dispose(disposing);
        }

        #region Windows Form Designer generated code

        /// <summary>
        ///  Required method for Designer support - do not modify
        ///  the contents of this method with the code editor.
        /// </summary>
        private void InitializeComponent()
        {
            System.ComponentModel.ComponentResourceManager resources = new System.ComponentModel.ComponentResourceManager(typeof(MainWindow));
            this.menuBar = new System.Windows.Forms.MenuStrip();
            this.fileFToolMenuItem = new System.Windows.Forms.ToolStripMenuItem();
            this.newProjectToolMenuItem = new System.Windows.Forms.ToolStripMenuItem();
            this.openProjectToolMenuItem = new System.Windows.Forms.ToolStripMenuItem();
            this.saveToolMenuItem = new System.Windows.Forms.ToolStripMenuItem();
            this.saveAsToolMenuItem1 = new System.Windows.Forms.ToolStripMenuItem();
            this.sepraratorToolStripMenuItem = new System.Windows.Forms.ToolStripSeparator();
            this.endToolMenuItem = new System.Windows.Forms.ToolStripMenuItem();
            this.editEToolMenuItem = new System.Windows.Forms.ToolStripMenuItem();
            this.helpToolMenuItem = new System.Windows.Forms.ToolStripMenuItem();
            this.versionToolMenuItem = new System.Windows.Forms.ToolStripMenuItem();
            this.scriptEditor = new System.Windows.Forms.RichTextBox();
            this.menuBar.SuspendLayout();
            this.SuspendLayout();
            // 
            // menuBar
            // 
            this.menuBar.Items.AddRange(new System.Windows.Forms.ToolStripItem[] {
            this.fileFToolMenuItem,
            this.editEToolMenuItem,
            this.helpToolMenuItem});
            this.menuBar.Location = new System.Drawing.Point(0, 0);
            this.menuBar.Name = "menuBar";
            this.menuBar.Size = new System.Drawing.Size(1184, 24);
            this.menuBar.TabIndex = 0;
            this.menuBar.Text = "menuBar";
            // 
            // fileFToolMenuItem
            // 
            this.fileFToolMenuItem.DropDownItems.AddRange(new System.Windows.Forms.ToolStripItem[] {
            this.newProjectToolMenuItem,
            this.openProjectToolMenuItem,
            this.saveToolMenuItem,
            this.saveAsToolMenuItem1,
            this.sepraratorToolStripMenuItem,
            this.endToolMenuItem});
            this.fileFToolMenuItem.Name = "fileFToolMenuItem";
            this.fileFToolMenuItem.Size = new System.Drawing.Size(54, 20);
            this.fileFToolMenuItem.Text = "File (&F)";
            // 
            // newProjectToolMenuItem
            // 
            this.newProjectToolMenuItem.Name = "newProjectToolMenuItem";
            this.newProjectToolMenuItem.ShortcutKeys = ((System.Windows.Forms.Keys)((System.Windows.Forms.Keys.Control | System.Windows.Forms.Keys.N)));
            this.newProjectToolMenuItem.Size = new System.Drawing.Size(205, 22);
            this.newProjectToolMenuItem.Text = "New Project (&N)";
            this.newProjectToolMenuItem.Click += new System.EventHandler(this.newProjectToolMenuItem_Click);
            // 
            // openProjectToolMenuItem
            // 
            this.openProjectToolMenuItem.Name = "openProjectToolMenuItem";
            this.openProjectToolMenuItem.ShortcutKeys = ((System.Windows.Forms.Keys)((System.Windows.Forms.Keys.Control | System.Windows.Forms.Keys.O)));
            this.openProjectToolMenuItem.Size = new System.Drawing.Size(205, 22);
            this.openProjectToolMenuItem.Text = "Open Project (&O)";
            this.openProjectToolMenuItem.Click += new System.EventHandler(this.openProjectToolMenuItem_Click);
            // 
            // saveToolMenuItem
            // 
            this.saveToolMenuItem.Name = "saveToolMenuItem";
            this.saveToolMenuItem.ShortcutKeys = ((System.Windows.Forms.Keys)((System.Windows.Forms.Keys.Control | System.Windows.Forms.Keys.S)));
            this.saveToolMenuItem.Size = new System.Drawing.Size(205, 22);
            this.saveToolMenuItem.Text = "Save (&S)";
            // 
            // saveAsToolMenuItem1
            // 
            this.saveAsToolMenuItem1.Name = "saveAsToolMenuItem1";
            this.saveAsToolMenuItem1.ShortcutKeys = ((System.Windows.Forms.Keys)(((System.Windows.Forms.Keys.Control | System.Windows.Forms.Keys.Shift) 
            | System.Windows.Forms.Keys.S)));
            this.saveAsToolMenuItem1.Size = new System.Drawing.Size(205, 22);
            this.saveAsToolMenuItem1.Text = "Save as (&A)";
            // 
            // sepraratorToolStripMenuItem
            // 
            this.sepraratorToolStripMenuItem.Name = "sepraratorToolStripMenuItem";
            this.sepraratorToolStripMenuItem.Size = new System.Drawing.Size(202, 6);
            // 
            // endToolMenuItem
            // 
            this.endToolMenuItem.Name = "endToolMenuItem";
            this.endToolMenuItem.Size = new System.Drawing.Size(205, 22);
            this.endToolMenuItem.Text = "End (&X)";
            this.endToolMenuItem.Click += new System.EventHandler(this.endToolMenuItem_Click);
            // 
            // editEToolMenuItem
            // 
            this.editEToolMenuItem.Name = "editEToolMenuItem";
            this.editEToolMenuItem.Size = new System.Drawing.Size(56, 20);
            this.editEToolMenuItem.Text = "Edit (&E)";
            this.editEToolMenuItem.Click += new System.EventHandler(this.editToolMenuItem_Click);
            // 
            // helpToolMenuItem
            // 
            this.helpToolMenuItem.DropDownItems.AddRange(new System.Windows.Forms.ToolStripItem[] {
            this.versionToolMenuItem});
            this.helpToolMenuItem.Name = "helpToolMenuItem";
            this.helpToolMenuItem.Size = new System.Drawing.Size(64, 20);
            this.helpToolMenuItem.Text = "Help (&H)";
            // 
            // versionToolMenuItem
            // 
            this.versionToolMenuItem.Name = "versionToolMenuItem";
            this.versionToolMenuItem.ShowShortcutKeys = false;
            this.versionToolMenuItem.Size = new System.Drawing.Size(124, 22);
            this.versionToolMenuItem.Text = "Version (&A)";
            this.versionToolMenuItem.Click += new System.EventHandler(this.versionToolMenuItem_Click);
            // 
            // scriptEditor
            // 
            this.scriptEditor.Location = new System.Drawing.Point(764, 27);
            this.scriptEditor.Name = "scriptEditor";
            this.scriptEditor.Size = new System.Drawing.Size(420, 834);
            this.scriptEditor.TabIndex = 1;
            this.scriptEditor.Text = "";
            // 
            // MainWindow
            // 
            this.AutoScaleDimensions = new System.Drawing.SizeF(7F, 15F);
            this.AutoScaleMode = System.Windows.Forms.AutoScaleMode.Font;
            this.ClientSize = new System.Drawing.Size(1184, 861);
            this.Controls.Add(this.scriptEditor);
            this.Controls.Add(this.menuBar);
            this.Icon = ((System.Drawing.Icon)(resources.GetObject("$this.Icon")));
            this.MainMenuStrip = this.menuBar;
            this.Name = "MainWindow";
            this.Text = "Hoi4 Modding Supporter";
            this.menuBar.ResumeLayout(false);
            this.menuBar.PerformLayout();
            this.ResumeLayout(false);
            this.PerformLayout();

        }

        #endregion

        private System.Windows.Forms.MenuStrip menuBar;
        private System.Windows.Forms.ToolStripMenuItem fileFToolMenuItem;
        private System.Windows.Forms.ToolStripMenuItem newProjectToolMenuItem;
        private System.Windows.Forms.ToolStripMenuItem openProjectToolMenuItem;
        private System.Windows.Forms.ToolStripMenuItem saveToolMenuItem;
        private System.Windows.Forms.ToolStripMenuItem editEToolMenuItem;
        private System.Windows.Forms.ToolStripMenuItem helpToolMenuItem;
        private System.Windows.Forms.ToolStripMenuItem versionToolMenuItem;
        private System.Windows.Forms.ToolStripSeparator sepraratorToolStripMenuItem;
        private System.Windows.Forms.ToolStripMenuItem endToolMenuItem;
        private System.Windows.Forms.ToolStripMenuItem saveAsToolMenuItem1;
        private System.Windows.Forms.RichTextBox scriptEditor;
    }
}

